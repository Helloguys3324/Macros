use crate::automation::{self, AutomationHandle};
use crate::config;
use crate::models::{AppConfig, Point, Roi};
use eframe::egui;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};

pub struct ClanTrackerApp {
    cfg: AppConfig,
    members_text: String,
    logs: VecDeque<String>,
    log_tx: Sender<String>,
    log_rx: Receiver<String>,
    worker: Option<AutomationHandle>,
    overlay_rx: std::sync::mpsc::Receiver<OverlayEvent>,
    overlay_tx: std::sync::mpsc::Sender<OverlayEvent>,
}

impl ClanTrackerApp {
    pub fn new() -> Self {
        let mut cfg = config::load_config();
        let _ = config::ensure_members_file(&cfg.members_file);
        let mut members_text = cfg.members.join("\n");
        if members_text.trim().is_empty() {
            if let Ok(from_file) = config::load_members_file(&cfg.members_file) {
                if !from_file.is_empty() {
                    cfg.members = from_file.clone();
                    members_text = from_file.join("\n");
                }
            }
        }
        let (log_tx, log_rx) = mpsc::channel();
        let (tx, rx) = mpsc::channel();

        Self {
            cfg,
            members_text,
            logs: VecDeque::new(),
            log_tx,
            log_rx,
            worker: None,
            overlay_rx: rx,
            overlay_tx: tx,
        }
    }

    fn pull_logs(&mut self) {
        while let Ok(line) = self.log_rx.try_recv() {
            self.push_log(line);
        }
    }

    fn push_log(&mut self, line: impl Into<String>) {
        self.logs.push_back(line.into());
        while self.logs.len() > 500 {
            self.logs.pop_front();
        }
    }

    fn start_automation(&mut self) {
        self.cfg.members = self
            .members_text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        if self.cfg.members.is_empty() {
            if let Ok(from_file) = config::load_members_file(&self.cfg.members_file) {
                if !from_file.is_empty() {
                    self.cfg.members = from_file.clone();
                    self.members_text = from_file.join("\n");
                    self.push_log("Members loaded from names file.");
                }
            }
        }

        if self.cfg.members.is_empty() {
            self.push_log("Cannot start: members list is empty.");
            return;
        }

        if self.cfg.search_field.is_none() {
            self.push_log("Cannot start: set Search Field first.");
            return;
        }

        if self.cfg.number_roi.is_none() {
            self.push_log("Cannot start: set Points ROI first.");
            return;
        }

        if let Err(err) = config::save_members_file(&self.cfg.members_file, &self.cfg.members) {
            self.push_log(format!("Names file save warning: {}", err));
        }

        if let Err(err) = config::save_config(&self.cfg) {
            self.push_log(format!("Config save warning: {}", err));
        }

        match automation::start(self.cfg.clone(), self.log_tx.clone()) {
            Ok(handle) => {
                self.worker = Some(handle);
                self.push_log("Automation started.");
            }
            Err(err) => self.push_log(format!("Failed to start automation: {}", err)),
        }
    }

    fn stop_automation(&mut self) {
        if let Some(mut handle) = self.worker.take() {
            handle.stop();
            self.push_log("Automation stopped.");
        }
    }

    fn process_overlay_events(&mut self) {
        while let Ok(event) = self.overlay_rx.try_recv() {
            match event {
                OverlayEvent::PointSelected(point) => {
                    self.cfg.search_field = Some(point);
                    self.push_log(format!("Search field point set to ({}, {}).", point.x, point.y));
                }
                OverlayEvent::RoiSelected(roi) => {
                    self.cfg.number_roi = Some(roi);
                    self.push_log(format!(
                        "ROI set to x={} y={} w={} h={}.",
                        roi.x, roi.y, roi.w, roi.h
                    ));
                }
                OverlayEvent::Error(err) => {
                    self.push_log(format!("Overlay error: {}", err));
                }
            }
        }
    }
}

impl eframe::App for ClanTrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pull_logs();
        self.process_overlay_events();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Clan Tracking Bot");
            ui.add_space(6.0);

            ui.colored_label(
                egui::Color32::YELLOW,
                egui::RichText::new(
                    "⚠️ REQUIRED: Set game to Borderless Windowed mode or screen capture will be black!",
                )
                .strong(),
            );

            ui.separator();
            ui.label("Clan Members (one name per line):");
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.members_text).desired_rows(10).desired_width(f32::INFINITY));
            });
            ui.label("Names File Path:");
            ui.text_edit_singleline(&mut self.cfg.members_file);
            ui.horizontal(|ui| {
                if ui.button("Load Names File").clicked() {
                    match config::load_members_file(&self.cfg.members_file) {
                        Ok(members) => {
                            self.cfg.members = members.clone();
                            self.members_text = members.join("\n");
                            self.push_log(format!(
                                "Loaded {} names from file.",
                                self.cfg.members.len()
                            ));
                        }
                        Err(err) => self.push_log(format!("Load names file failed: {}", err)),
                    }
                }
                if ui.button("Save Names File").clicked() {
                    let members = self
                        .members_text
                        .lines()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>();
                    match config::save_members_file(&self.cfg.members_file, &members) {
                        Ok(()) => self.push_log(format!("Saved {} names to file.", members.len())),
                        Err(err) => self.push_log(format!("Save names file failed: {}", err)),
                    }
                }
            });

            ui.separator();
            ui.label("Game Window Title (exact):");
            ui.text_edit_singleline(&mut self.cfg.game_window_title);

            ui.horizontal(|ui| {
                ui.label("Scan Delay (ms):");
                ui.add(egui::DragValue::new(&mut self.cfg.scan_delay_ms).range(100..=10_000));
                ui.label("Loop Interval (sec):");
                ui.add(egui::DragValue::new(&mut self.cfg.interval_secs).range(30..=86_400));
            });

            ui.add(
                egui::Slider::new(&mut self.cfg.ocr_threshold, 0..=255)
                    .text("OCR Contrast Threshold"),
            );

            ui.horizontal(|ui| {
                if ui.button("Select Search Point").clicked() {
                    let tx = self.overlay_tx.clone();
                    std::thread::spawn(move || {
                        match crate::overlay::select_search_field_point() {
                            Ok(point) => { let _ = tx.send(OverlayEvent::PointSelected(point)); }
                            Err(e) => { let _ = tx.send(OverlayEvent::Error(e.to_string())); }
                        }
                    });
                }

                if ui.button("Select Points Area (ROI)").clicked() {
                    let tx = self.overlay_tx.clone();
                    std::thread::spawn(move || {
                        match crate::overlay::select_number_roi_rect() {
                            Ok(roi) => { let _ = tx.send(OverlayEvent::RoiSelected(roi)); }
                            Err(e) => { let _ = tx.send(OverlayEvent::Error(e.to_string())); }
                        }
                    });
                }
            });

            if let Some(point) = self.cfg.search_field {
                ui.label(format!("Search Field: ({}, {})", point.x, point.y));
            } else {
                ui.label("Search Field: Not set");
            }

            if let Some(roi) = self.cfg.number_roi {
                ui.label(format!(
                    "Points ROI: x={} y={} w={} h={}",
                    roi.x, roi.y, roi.w, roi.h
                ));
            } else {
                ui.label("Points ROI: Not set");
            }

            ui.separator();
            ui.horizontal(|ui| {
                if self.worker.is_none() {
                    let mut can_start =
                        self.cfg.search_field.is_some() && self.cfg.number_roi.is_some();
                    let mut err_msg = "Set Search Field and ROI before start.";

                    if let Some(roi) = self.cfg.number_roi {
                        if roi.w < 10 || roi.h < 10 {
                            can_start = false;
                            err_msg = "ROI is too small. Please recapture a larger rectangle.";
                        }
                    }

                    if ui
                        .add_enabled(can_start, egui::Button::new("Start Automation"))
                        .clicked()
                    {
                        self.start_automation();
                    }
                    if !can_start {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            err_msg,
                        );
                    }
                } else if ui.button("Stop").clicked() {
                    self.stop_automation();
                }
            });

            ui.separator();
            ui.label("Live Log:");
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(260.0)
                .show(ui, |ui| {
                    for line in &self.logs {
                        ui.label(line);
                    }
                });
        });
    }
}

impl Drop for ClanTrackerApp {
    fn drop(&mut self) {
        self.stop_automation();
        let members = self
            .members_text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let _ = config::save_members_file(&self.cfg.members_file, &members);
        let _ = config::save_config(&self.cfg);
    }
}

enum OverlayEvent {
    PointSelected(Point),
    RoiSelected(Roi),
    Error(String),
}

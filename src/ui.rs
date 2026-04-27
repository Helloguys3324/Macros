use crate::automation::{self, AutomationHandle};
use crate::config;
use crate::models::AppConfig;
use crate::overlay;
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
}

impl ClanTrackerApp {
    pub fn new() -> Self {
        let cfg = config::load_config();
        let members_text = cfg.members.join("\n");
        let (log_tx, log_rx) = mpsc::channel();

        Self {
            cfg,
            members_text,
            logs: VecDeque::new(),
            log_tx,
            log_rx,
            worker: None,
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
            self.push_log("Cannot start: members list is empty.");
            return;
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
}

impl eframe::App for ClanTrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pull_logs();

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
            ui.add(egui::TextEdit::multiline(&mut self.members_text).desired_rows(10));

            ui.separator();
            ui.label("Discord Webhook URL:");
            ui.text_edit_singleline(&mut self.cfg.webhook_url);

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
                if ui.button("Set Search Field Position").clicked() {
                    match overlay::select_search_field_point() {
                        Ok(point) => {
                            self.cfg.search_field = Some(point);
                            self.push_log(format!(
                                "Search field point set to ({}, {}).",
                                point.x, point.y
                            ));
                        }
                        Err(err) => self.push_log(format!(
                            "Search field selection failed: {}",
                            err
                        )),
                    }
                }

                if ui.button("Set Number ROI Box").clicked() {
                    match overlay::select_number_roi_rect() {
                        Ok(roi) => {
                            self.cfg.number_roi = Some(roi);
                            self.push_log(format!(
                                "ROI set to x={} y={} w={} h={}.",
                                roi.x, roi.y, roi.w, roi.h
                            ));
                        }
                        Err(err) => self.push_log(format!("ROI selection failed: {}", err)),
                    }
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
                    if ui.button("Start Automation").clicked() {
                        self.start_automation();
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
        let _ = config::save_config(&self.cfg);
    }
}


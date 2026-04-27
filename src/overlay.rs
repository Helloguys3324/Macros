use crate::models::{Point, Roi};
use anyhow::{anyhow, bail, Result};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::env;
use std::process::Command;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy)]
enum OverlayMode {
    Point,
    Rect,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(tag = "kind")]
enum OverlayResult {
    Point { x: i32, y: i32 },
    Rect { x: u32, y: u32, w: u32, h: u32 },
}

pub fn try_run_overlay_from_cli() -> Result<bool> {
    let args: Vec<String> = env::args().collect();
    if args.len() >= 3 && args[1] == "--overlay" {
        match args[2].as_str() {
            "point" => run_overlay(OverlayMode::Point)?,
            "rect" => run_overlay(OverlayMode::Rect)?,
            other => bail!("Unknown overlay mode: {}", other),
        }
        return Ok(true);
    }
    Ok(false)
}

pub fn select_search_field_point() -> Result<Point> {
    let result = run_overlay_subprocess("point")?;
    match result {
        OverlayResult::Point { x, y } => Ok(Point { x, y }),
        _ => bail!("Unexpected overlay output for point selector"),
    }
}

pub fn select_number_roi_rect() -> Result<Roi> {
    let result = run_overlay_subprocess("rect")?;
    match result {
        OverlayResult::Rect { x, y, w, h } => Ok(Roi { x, y, w, h }),
        _ => bail!("Unexpected overlay output for rectangle selector"),
    }
}

fn run_overlay_subprocess(mode: &str) -> Result<OverlayResult> {
    let exe = env::current_exe()?;
    let output = Command::new(exe)
        .arg("--overlay")
        .arg(mode)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Overlay process failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| anyhow!("Overlay process returned empty output"))?;

    Ok(serde_json::from_str::<OverlayResult>(line.trim())?)
}

fn run_overlay(mode: OverlayMode) -> Result<()> {
    let shared_result: Arc<Mutex<Option<OverlayResult>>> = Arc::new(Mutex::new(None));
    let result_clone = Arc::clone(&shared_result);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_fullscreen(true)
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "Overlay Selector",
        native_options,
        Box::new(move |_cc| Ok(Box::new(OverlayApp::new(mode, Arc::clone(&result_clone))))),
    )
    .map_err(|e| anyhow!(e.to_string()))?;

    if let Some(result) = *shared_result.lock().map_err(|_| anyhow!("Overlay mutex poisoned"))? {
        println!("{}", serde_json::to_string(&result)?);
        Ok(())
    } else {
        bail!("Overlay selection canceled")
    }
}

struct OverlayApp {
    mode: OverlayMode,
    shared_result: Arc<Mutex<Option<OverlayResult>>>,
    drag_start: Option<egui::Pos2>,
    drag_current: Option<egui::Pos2>,
}

impl OverlayApp {
    fn new(mode: OverlayMode, shared_result: Arc<Mutex<Option<OverlayResult>>>) -> Self {
        Self {
            mode,
            shared_result,
            drag_start: None,
            drag_current: None,
        }
    }
}

impl eframe::App for OverlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let full = ui.max_rect();
            ui.painter()
                .rect_filled(full, 0.0, egui::Color32::from_black_alpha(70));

            let pointer_pos = ctx.input(|i| i.pointer.interact_pos());

            match self.mode {
                OverlayMode::Point => {
                    if let Some(pos) = pointer_pos {
                        let radius = 8.0;
                        ui.painter().circle_stroke(
                            pos,
                            radius,
                            egui::Stroke::new(2.0, egui::Color32::LIGHT_GREEN),
                        );
                    }

                    if ctx.input(|i| i.pointer.primary_clicked()) {
                        if let Some(pos) = pointer_pos {
                            if let Ok(mut guard) = self.shared_result.lock() {
                                *guard = Some(OverlayResult::Point {
                                    x: pos.x.round() as i32,
                                    y: pos.y.round() as i32,
                                });
                            }
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
                OverlayMode::Rect => {
                    if ctx.input(|i| i.pointer.primary_pressed()) {
                        self.drag_start = pointer_pos;
                        self.drag_current = pointer_pos;
                    }

                    if ctx.input(|i| i.pointer.primary_down()) {
                        self.drag_current = pointer_pos;
                    }

                    if let (Some(a), Some(b)) = (self.drag_start, self.drag_current) {
                        let rect = egui::Rect::from_two_pos(a, b);
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            egui::Stroke::new(2.0, egui::Color32::GREEN),
                        );
                    }

                    if ctx.input(|i| i.pointer.primary_released()) {
                        if let (Some(a), Some(b)) = (self.drag_start, self.drag_current) {
                            let left = a.x.min(b.x).max(0.0);
                            let top = a.y.min(b.y).max(0.0);
                            let w = (a.x - b.x).abs();
                            let h = (a.y - b.y).abs();
                            if w >= 3.0 && h >= 3.0 {
                                if let Ok(mut guard) = self.shared_result.lock() {
                                    *guard = Some(OverlayResult::Rect {
                                        x: left.round() as u32,
                                        y: top.round() as u32,
                                        w: w.round() as u32,
                                        h: h.round() as u32,
                                    });
                                }
                            }
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }

            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                ui.add_space(20.0);
                match self.mode {
                    OverlayMode::Point => {
                        ui.label(
                            egui::RichText::new("Click to set Search Field Position (Esc to cancel)")
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                    }
                    OverlayMode::Rect => {
                        ui.label(
                            egui::RichText::new(
                                "Drag to set Points ROI Rectangle (Esc to cancel)",
                            )
                            .color(egui::Color32::WHITE)
                            .strong(),
                        );
                    }
                }
            });
        });
    }
}


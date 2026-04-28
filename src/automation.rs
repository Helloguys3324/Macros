use crate::background::BackgroundInput;
use crate::capture::ScreenCapture;
use crate::config;
use crate::discord;
use crate::models::{AppConfig, MemberScan, PointsState, ScanSummary};
use crate::ocr::OcrEngine;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::runtime::Runtime;

pub struct AutomationHandle {
    stop_flag: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl AutomationHandle {
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn start(cfg: AppConfig, log_tx: Sender<String>) -> Result<AutomationHandle> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop_flag);

    let join = thread::spawn(move || {
        if let Err(err) = run_automation_loop(cfg, log_tx.clone(), stop_clone) {
            let _ = log_tx.send(format!("Automation fatal error: {}", err));
        }
    });

    Ok(AutomationHandle {
        stop_flag,
        join: Some(join),
    })
}

fn run_automation_loop(
    cfg: AppConfig,
    log_tx: Sender<String>,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    let mut points_state: PointsState = config::load_points_state();
    let mut capture = match ScreenCapture::new_primary() {
        Ok(capture) => capture,
        Err(err) => {
            send_log(&log_tx, format!("Screen capture init failed: {}", err));
            return Ok(());
        }
    };
    send_log(
        &log_tx,
        format!("Screen capture backend: {}", capture.backend_name()),
    );
    let model_path = config::resolve_app_relative(&cfg.model_path);
    let dict_path = config::resolve_app_relative(&cfg.dict_path);
    let model_path_str = model_path.to_string_lossy().to_string();
    let dict_path_str = dict_path.to_string_lossy().to_string();
    let mut ocr = loop {
        match OcrEngine::new(&model_path_str, &dict_path_str) {
            Ok(ocr) => break ocr,
            Err(err) => {
                send_log(
                    &log_tx,
                    format!("OCR init failed: {}. Retrying in 5s...", err),
                );
                sleep_with_stop(Duration::from_secs(5), &stop_flag);
                if stop_flag.load(Ordering::Relaxed) {
                    return Ok(());
                }
            }
        }
    };
    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(err) => {
            send_log(&log_tx, format!("Tokio runtime init failed: {}", err));
            return Ok(());
        }
    };

    send_log(&log_tx, "Automation thread started.");

    while !stop_flag.load(Ordering::Relaxed) {
        let background = match BackgroundInput::connect(&cfg.game_window_title) {
            Ok(bg) => bg,
            Err(err) => {
                send_log(&log_tx, format!("Game window not ready: {}", err));
                sleep_with_stop(Duration::from_secs(2), &stop_flag);
                continue;
            }
        };

        let mut rows = Vec::with_capacity(cfg.members.len());
        let mut total_points_gained = 0u32;

        for name in &cfg.members {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            send_log(&log_tx, format!("Scanning {}...", name));
            let prev = *points_state.last_points.get(name).unwrap_or(&0);
            let mut now = prev;

            if let Some(search) = cfg.search_field {
                if let Err(err) = background.click_search_field(search.x, search.y) {
                    send_log(&log_tx, format!("Click failed for {}: {}", name, err));
                } else {
                    sleep_with_stop(Duration::from_millis(250), &stop_flag);
                    let _ = background.clear_search_field();
                    sleep_with_stop(Duration::from_millis(250), &stop_flag);
                    let clean_name: String = name
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    let raw_bytes: Vec<u16> = clean_name.encode_utf16().collect();
                    send_log(&log_tx, format!("Raw name bytes for input: {:?}", raw_bytes));

                    let _ = background.type_text(&clean_name);
                    sleep_with_stop(Duration::from_millis(1200), &stop_flag);
                    let _ = background.press_backspace();
                    sleep_with_stop(Duration::from_millis(150), &stop_flag);
                    let _ = background.press_enter();
                }
            } else {
                send_log(&log_tx, "Search field point is not configured.");
            }

            sleep_with_stop(Duration::from_millis(cfg.scan_delay_ms), &stop_flag);
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            if let Some(roi) = cfg.number_roi {
                match capture.capture_roi_grayscale(roi) {
                    Ok(gray) => match ocr.read_points(&gray, roi.w, roi.h, cfg.ocr_threshold) {
                        Ok(Some(parsed)) => now = parsed,
                        Ok(None) => {
                            send_log(
                                &log_tx,
                                format!(
                                    "OCR warning for {}: unreadable text, keeping previous value ({})",
                                    name, prev
                                ),
                            );
                        }
                        Err(err) => {
                            send_log(
                                &log_tx,
                                format!(
                                    "OCR warning for {}: {}, keeping previous value ({})",
                                    name, err, prev
                                ),
                            );
                        }
                    },
                    Err(err) => {
                        send_log(
                            &log_tx,
                            format!("Screen capture failed for {}: {}", name, err),
                        );
                    }
                }
            } else {
                send_log(&log_tx, "Points ROI rectangle is not configured.");
            }

            let gained = now.saturating_sub(prev);
            let online = now != prev;
            total_points_gained = total_points_gained.saturating_add(gained);
            points_state.last_points.insert(name.clone(), now);

            rows.push(MemberScan {
                name: name.clone(),
                prev_points: prev,
                now_points: now,
                gained_points: gained,
                online,
            });
        }

        let summary = ScanSummary {
            rows,
            total_points_gained,
        };

        if !cfg.webhook_url.trim().is_empty() {
            let result = runtime.block_on(discord::send_summary(&cfg.webhook_url, &summary));
            if let Err(err) = result {
                send_log(&log_tx, format!("Webhook error: {}", err));
            } else {
                send_log(
                    &log_tx,
                    format!(
                        "Webhook sent. Total clan points gained: {}",
                        summary.total_points_gained
                    ),
                );
            }
        } else {
            send_log(&log_tx, "Webhook URL is empty, skipping Discord send.");
        }

        if let Err(err) = config::save_points_state(&points_state) {
            send_log(
                &log_tx,
                format!("Failed to save points_state.json: {}", err),
            );
        }

        send_log(
            &log_tx,
            format!(
                "Cycle completed. Waiting {} seconds for next scan.",
                cfg.interval_secs
            ),
        );
        sleep_with_stop(Duration::from_secs(cfg.interval_secs), &stop_flag);
    }

    send_log(&log_tx, "Automation thread stopped.");
    Ok(())
}

fn send_log(log_tx: &Sender<String>, msg: impl Into<String>) {
    let _ = log_tx.send(msg.into());
}

fn sleep_with_stop(total: Duration, stop_flag: &AtomicBool) {
    let mut slept = Duration::from_millis(0);
    let step = Duration::from_millis(200);
    while slept < total && !stop_flag.load(Ordering::Relaxed) {
        thread::sleep(step);
        slept += step;
    }
}

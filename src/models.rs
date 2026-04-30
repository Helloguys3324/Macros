use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Roi {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub members: Vec<String>,
    pub members_file: String,
    pub game_window_title: String,
    pub search_field: Option<Point>,
    pub number_roi: Option<Roi>,
    pub scan_delay_ms: u64,
    pub interval_secs: u64,
    pub ocr_threshold: u8,
    pub model_path: String,
    pub dict_path: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            members: Vec::new(),
            members_file: "members.txt".to_string(),
            game_window_title: "Roblox".to_string(),
            search_field: None,
            number_roi: None,
            scan_delay_ms: 500,
            interval_secs: 420,
            ocr_threshold: 150,
            model_path: "models/ch_PP-OCRv4_rec_server_infer.onnx".to_string(),
            dict_path: "models/ch_dict.txt".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PointsState {
    pub last_points: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
pub struct MemberScan {
    pub name: String,
    pub prev_points: u32,
    pub now_points: u32,
    pub gained_points: u32,
    pub online: bool,
}

#[derive(Debug, Clone)]
pub struct ScanSummary {
    pub rows: Vec<MemberScan>,
    pub total_points_gained: u32,
}


use crate::models::{AppConfig, PointsState};
use anyhow::Result;
use std::fs;

const CONFIG_PATH: &str = "config.json";
const POINTS_STATE_PATH: &str = "points_state.json";

pub fn load_config() -> AppConfig {
    match fs::read_to_string(CONFIG_PATH) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(cfg: &AppConfig) -> Result<()> {
    let raw = serde_json::to_string_pretty(cfg)?;
    fs::write(CONFIG_PATH, raw)?;
    Ok(())
}

pub fn load_points_state() -> PointsState {
    match fs::read_to_string(POINTS_STATE_PATH) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => PointsState::default(),
    }
}

pub fn save_points_state(state: &PointsState) -> Result<()> {
    let raw = serde_json::to_string_pretty(state)?;
    fs::write(POINTS_STATE_PATH, raw)?;
    Ok(())
}


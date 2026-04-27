use crate::models::{AppConfig, PointsState};
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

const CONFIG_PATH: &str = "config.json";
const POINTS_STATE_PATH: &str = "points_state.json";

pub fn load_config() -> AppConfig {
    match fs::read_to_string(config_path()) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(cfg: &AppConfig) -> Result<()> {
    let raw = serde_json::to_string_pretty(cfg)?;
    fs::write(config_path(), raw)?;
    Ok(())
}

pub fn load_points_state() -> PointsState {
    match fs::read_to_string(points_state_path()) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => PointsState::default(),
    }
}

pub fn save_points_state(state: &PointsState) -> Result<()> {
    let raw = serde_json::to_string_pretty(state)?;
    fs::write(points_state_path(), raw)?;
    Ok(())
}

pub fn resolve_app_relative(path: &str) -> PathBuf {
    let path_buf = PathBuf::from(path);
    if path_buf.is_absolute() {
        path_buf
    } else {
        app_dir().join(path_buf)
    }
}

pub fn ensure_members_file(path: &str) -> Result<()> {
    let full_path = resolve_app_relative(path);
    if full_path.exists() {
        return Ok(());
    }
    fs::write(
        &full_path,
        "# One member name per line\n# Example:\n# PlayerOne\n# PlayerTwo\n",
    )
    .with_context(|| format!("Failed to create members file: {}", full_path.display()))?;
    Ok(())
}

pub fn load_members_file(path: &str) -> Result<Vec<String>> {
    let full_path = resolve_app_relative(path);
    if !full_path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&full_path)
        .with_context(|| format!("Failed to read members file: {}", full_path.display()))?;

    let members = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    Ok(members)
}

pub fn save_members_file(path: &str, members: &[String]) -> Result<()> {
    let full_path = resolve_app_relative(path);
    let body = if members.is_empty() {
        String::new()
    } else {
        format!("{}\n", members.join("\n"))
    };
    fs::write(&full_path, body)
        .with_context(|| format!("Failed to write members file: {}", full_path.display()))?;
    Ok(())
}

fn config_path() -> PathBuf {
    app_dir().join(CONFIG_PATH)
}

fn points_state_path() -> PathBuf {
    app_dir().join(POINTS_STATE_PATH)
}

fn app_dir() -> PathBuf {
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.to_path_buf();
        }
    }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}


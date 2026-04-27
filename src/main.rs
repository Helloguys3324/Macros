mod automation;
mod background;
mod capture;
mod config;
mod discord;
mod models;
mod ocr;
mod overlay;
mod ui;

use anyhow::Result;

fn main() -> Result<()> {
    if overlay::try_run_overlay_from_cli()? {
        return Ok(());
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Clan Tracking Bot",
        options,
        Box::new(|_cc| Ok(Box::new(ui::ClanTrackerApp::new()))),
    )?;

    Ok(())
}


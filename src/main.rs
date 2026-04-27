mod automation;
mod background;
mod capture;
mod config;
mod discord;
mod models;
mod ocr;
mod overlay;
mod ui;

fn main() {
    if let Err(err) = run() {
        eprintln!("fatal: {}", err);
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    if overlay::try_run_overlay_from_cli()? {
        return Ok(());
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Clan Tracking Bot",
        options,
        Box::new(|_cc| Ok(Box::new(ui::ClanTrackerApp::new()))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    Ok(())
}


use crate::models::ScanSummary;
use anyhow::Result;
use chrono::Local;

const WEBHOOK_URL: &str = "https://discord.com/api/webhooks/1498387529626288232/shZTKC3qAzqVGOahutESnGQjKnenoHnUvLleopX7b9SOnqkTTWMI8FFZ9L6AY8Q-X0ai";

pub async fn send_summary(_webhook_url: &str, summary: &ScanSummary) -> Result<()> {
    let now = Local::now();
    let timestamp = now.format("%Y-%m-%d %H:%M").to_string();
    let timezone = chrono::offset::Local::now().format("%z").to_string();
    
    let mut csv_content = String::from("Timestamp,Timezone,Roblox,Contribution\n");

    for row in &summary.rows {
        csv_content.push_str(&format!("{},{},{},{}\n", timestamp, timezone, row.name, row.gained_points));
    }

    let client = reqwest::Client::new();
    
    let form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::text(csv_content).file_name("export.csv"));

    client
        .post(WEBHOOK_URL)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

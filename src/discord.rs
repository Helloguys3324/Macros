use crate::models::ScanSummary;
use anyhow::Result;
use chrono::Local;
use reqwest::multipart;
use serde_json::json;

pub async fn send_summary(webhook_url: &str, summary: &ScanSummary) -> Result<()> {
    let mut online_lines = String::new();
    let mut offline_lines = String::new();

    let current_time = Local::now().format("%Y-%m-%d %H:%M").to_string();
    let mut csv_data = String::from("Timestamp,Roblox,Contribution\n");

    for row in &summary.rows {
        csv_data.push_str(&format!(
            "{},\"{}\",{}\n",
            current_time, row.name.replace('"', "\"\""), row.gained_points
        ));

        let line = format!(
            "{} | {} -> {} | +{}\n",
            row.name, row.prev_points, row.now_points, row.gained_points
        );
        if row.online {
            online_lines.push_str(&line);
        } else {
            offline_lines.push_str(&line);
        }
    }

    if online_lines.is_empty() {
        online_lines.push_str("No members detected online.\n");
    }
    if offline_lines.is_empty() {
        offline_lines.push_str("No members detected offline.\n");
    }

    let payload = json!({
        "embeds": [{
            "title": "Clan Tracking Report",
            "description": format!("**Total Clan Points Gained: {}**", summary.total_points_gained),
            "color": 0x00B0F4,
            "fields": [
                { "name": "🟢 ONLINE", "value": online_lines, "inline": false },
                { "name": "🔴 OFFLINE", "value": offline_lines, "inline": false }
            ],
            "footer": {
                "text": format!("Generated at {}", Local::now().format("%Y-%m-%d %H:%M:%S"))
            }
        }]
    });

    let form = multipart::Form::new()
        .text("payload_json", serde_json::to_string(&payload)?)
        .part("file", multipart::Part::text(csv_data).file_name("export.csv"));

    let client = reqwest::Client::new();
    client
        .post(webhook_url)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

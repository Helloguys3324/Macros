use crate::models::ScanSummary;
use anyhow::Result;
use chrono::{Utc, FixedOffset};
use reqwest::multipart;

const WEBHOOK_URL: &str = "https://discord.com/api/webhooks/1498387529626288232/shZTKC3qAzqVGOahutESnGQjKnenoHnUvLleopX7b9SOnqkTTWMI8FFZ9L6AY8Q-X0ai";

pub async fn send_summary(_webhook_url: &str, summary: &ScanSummary) -> Result<()> {
    let now = Utc::now();
    let gmt8 = FixedOffset::west_opt(8 * 3600).unwrap();
    let now_gmt8 = now.with_timezone(&gmt8);
    let current_time = now_gmt8.format("%Y-%m-%d %H:%M").to_string();
    let timezone = "GMT-8";
    
    let mut csv_data = String::from("Timestamp,Timezone,Roblox,Contribution\n");

    let mut changed_members = Vec::new();
    let mut unchanged_members = Vec::new();
    let mut not_in_guild = Vec::new();

    for row in &summary.rows {
        csv_data.push_str(&format!(
            "{},{},\"{}\",{}\n",
            current_time, timezone, row.name.replace('"', "\"\""), row.now_points
        ));

        let line = format!(
            "{} | {} -> {} | +{}\n",
            row.name, row.prev_points, row.now_points, row.gained_points
        );

        if row.now_points == 0 {
            not_in_guild.push(format!("{} | Not in Guild\n", row.name));
        } else if row.gained_points != 0 {
            changed_members.push(line.clone());
        } else {
            unchanged_members.push(line.clone());
        }
    }

    let mut full_txt_report = String::new();
    full_txt_report.push_str("--- MEMBERS WITH CHANGES ---\n");
    if changed_members.is_empty() {
        full_txt_report.push_str("None\n");
    } else {
        for m in changed_members { full_txt_report.push_str(&m); }
    }

    full_txt_report.push_str("\n--- MEMBERS WITH NO CHANGES ---\n");
    if unchanged_members.is_empty() {
        full_txt_report.push_str("None\n");
    } else {
        for m in unchanged_members { full_txt_report.push_str(&m); }
    }

    full_txt_report.push_str("\n--- NOT IN GUILD (0 POINTS) ---\n");
    if not_in_guild.is_empty() {
        full_txt_report.push_str("None\n");
    } else {
        for m in not_in_guild { full_txt_report.push_str(&m); }
    }

    let form = multipart::Form::new()
        .part("csv_file", multipart::Part::text(csv_data).file_name("export.csv").mime_str("text/csv")?)
        .part("txt_file", multipart::Part::text(full_txt_report).file_name("full_report.txt").mime_str("text/plain")?);

    let client = reqwest::Client::new();
    client
        .post(WEBHOOK_URL)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

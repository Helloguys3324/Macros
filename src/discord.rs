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

    let mut online_count = 0;
    let mut offline_count = 0;

    let mut changed_members = Vec::new();
    let mut unchanged_members = Vec::new();
    let mut not_in_guild = Vec::new();

    for row in &summary.rows {
        csv_data.push_str(&format!(
            "{},\"{}\",{}\n",
            current_time, row.name.replace('"', "\"\""), row.now_points
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

        if row.online {
            online_count += 1;
            if online_lines.len() + line.len() < 900 {
                online_lines.push_str(&line);
            }
        } else {
            offline_count += 1;
            if offline_lines.len() + line.len() < 900 {
                offline_lines.push_str(&line);
            }
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

    if online_count > 0 && online_lines.len() >= 900 {
        online_lines.push_str("\n... (see attached txt for full list)");
    }
    if offline_count > 0 && offline_lines.len() >= 900 {
        offline_lines.push_str("\n... (see attached txt for full list)");
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
                { "name": format!("🟢 ONLINE ({})", online_count), "value": online_lines, "inline": false },
                { "name": format!("🔴 OFFLINE ({})", offline_count), "value": offline_lines, "inline": false }
            ],
            "footer": {
                "text": format!("Generated at {}", Local::now().format("%Y-%m-%d %H:%M:%S"))
            }
        }]
    });

    let form = multipart::Form::new()
        .text("payload_json", serde_json::to_string(&payload)?)
        .part("csv_file", multipart::Part::text(csv_data).file_name("export.csv").mime_str("text/csv")?)
        .part("txt_file", multipart::Part::text(full_txt_report).file_name("full_report.txt").mime_str("text/plain")?);

    let client = reqwest::Client::new();
    client
        .post(webhook_url)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

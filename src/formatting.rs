use std::time::{SystemTime, UNIX_EPOCH};

/// Formats a Unix timestamp into a relative time string (e.g., "2 hours ago")
pub fn format_report_timestamp(timestamp: u64) -> String {
    let datetime = UNIX_EPOCH + std::time::Duration::from_secs(timestamp);
    let now = SystemTime::now();
    
    // Calculate time difference
    if let Ok(duration) = now.duration_since(datetime) {
        let days = duration.as_secs() / 86400;
        let hours = (duration.as_secs() % 86400) / 3600;
        let minutes = (duration.as_secs() % 3600) / 60;
        
        let relative = if days > 0 {
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        } else if hours > 0 {
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else if minutes > 0 {
            format!("{} minute{} ago", minutes, if minutes == 1 { "" } else { "s" })
        } else {
            "Just now".to_string()
        };
        
        relative
    } else {
        "Unknown".to_string()
    }
}

/// Formats a log filename timestamp (e.g., "20251010-222255.zevtc") into a readable format
pub fn format_timestamp(filename: &str) -> Option<String> {
    // Extract timestamp from filename like "20251010-222255.zevtc"
    let parts: Vec<&str> = filename.split('-').collect();
    if parts.len() < 2 {
        return None;
    }
    
    let date_part = parts[0];
    let time_part = parts[1].split('.').next()?;
    
    if date_part.len() != 8 || time_part.len() != 6 {
        return None;
    }
    
    // Parse date: YYYYMMDD
    let year = date_part[0..4].parse::<i32>().ok()?;
    let month = date_part[4..6].parse::<u32>().ok()?;
    let day = date_part[6..8].parse::<u32>().ok()?;
    
    // Parse time: HHMMSS
    let hour = time_part[0..2].parse::<u32>().ok()?;
    let minute = time_part[2..4].parse::<u32>().ok()?;
    
    // Format month name
    let month_name = match month {
        1 => "Jan", 2 => "Feb", 3 => "Mar", 4 => "Apr",
        5 => "May", 6 => "Jun", 7 => "Jul", 8 => "Aug",
        9 => "Sep", 10 => "Oct", 11 => "Nov", 12 => "Dec",
        _ => return None,
    };
    
    Some(format!(
        "{} {}, {} - {:02}:{:02}",
        month_name, day, year, hour, minute
    ))
}
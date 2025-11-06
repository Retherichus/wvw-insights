use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{create_dir_all, File};
use std::path::Path;
use std::sync::{LazyLock, Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedLogs {
    pub filenames: HashSet<String>,
}

impl UploadedLogs {
    pub fn get() -> MutexGuard<'static, Self> {
        UPLOADED_LOGS.lock().unwrap()
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        log::info!("Loading uploaded logs from: {:?}", path);
        
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let uploaded: Self = serde_json::from_str(&contents)?;
            log::info!("Loaded {} previously uploaded logs", uploaded.filenames.len());
            *UPLOADED_LOGS.lock().unwrap() = uploaded;
        } else {
            log::info!("No uploaded logs file exists yet, starting fresh");
        }
        Ok(())
    }

    pub fn store(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let prefix = path.parent().unwrap();
        create_dir_all(prefix)?;
        let mut file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        log::info!("Saved {} uploaded logs to disk", self.filenames.len());
        Ok(())
    }

    pub fn add_log(&mut self, filename: String) {
        self.filenames.insert(filename);
    }

    pub fn is_uploaded(&self, filename: &str) -> bool {
        self.filenames.contains(filename)
    }

    pub fn clear(&mut self) {
        self.filenames.clear();
    }

    /// Removes uploaded log entries older than 72 hours
    /// Returns the number of entries removed
    pub fn cleanup_old_entries(&mut self) -> usize {
        let cutoff_time = std::time::SystemTime::now()
            - std::time::Duration::from_secs(72 * 60 * 60); // 72 hours in seconds

        let initial_count = self.filenames.len();
        
        // Filter out logs older than 72 hours
        self.filenames.retain(|filename| {
            // Parse timestamp from filename (format: YYYYMMDD-HHMMSS)
            if let Some(timestamp_str) = extract_timestamp_from_filename(filename) {
                if let Some(log_time) = parse_log_timestamp(&timestamp_str) {
                    // Keep the log if it's newer than cutoff
                    return log_time >= cutoff_time;
                }
            }
            
            // If we can't parse the timestamp, keep it to be safe
            true
        });

        let removed_count = initial_count - self.filenames.len();
        
        if removed_count > 0 {
            log::info!(
                "Cleaned up {} uploaded log entries older than 72 hours ({} remaining)",
                removed_count,
                self.filenames.len()
            );
        }
        
        removed_count
    }
}

/// Extracts the timestamp portion from a log filename
/// Example: "20241105-143022.zevtc" -> "20241105-143022"
fn extract_timestamp_from_filename(filename: &str) -> Option<String> {
    // Remove any path separators and get just the filename
    let filename = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    
    // Remove the extension if present
    let without_ext = filename.strip_suffix(".zevtc").unwrap_or(filename);
    
    // The timestamp should be the first part (YYYYMMDD-HHMMSS)
    // It's 15 characters long: 8 for date + 1 for dash + 6 for time
    if without_ext.len() >= 15 {
        Some(without_ext[..15].to_string())
    } else {
        None
    }
}

/// Parses a log timestamp string into SystemTime
/// Format: YYYYMMDD-HHMMSS
fn parse_log_timestamp(timestamp: &str) -> Option<std::time::SystemTime> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    
    // Parse the timestamp: YYYYMMDD-HHMMSS
    if timestamp.len() != 15 {
        return None;
    }
    
    let year = timestamp[..4].parse::<i32>().ok()?;
    let month = timestamp[4..6].parse::<u32>().ok()?;
    let day = timestamp[6..8].parse::<u32>().ok()?;
    let hour = timestamp[9..11].parse::<u32>().ok()?;
    let minute = timestamp[11..13].parse::<u32>().ok()?;
    let second = timestamp[13..15].parse::<u32>().ok()?;
    
    // Create NaiveDateTime
    let naive_dt = NaiveDateTime::parse_from_str(
        &format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hour, minute, second),
        "%Y-%m-%d %H:%M:%S"
    ).ok()?;
    
    // Convert to UTC and then to SystemTime
    let utc_dt = Utc.from_utc_datetime(&naive_dt);
    Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(utc_dt.timestamp() as u64))
}

// Use LazyLock to lazily initialize the static
static UPLOADED_LOGS: LazyLock<Mutex<UploadedLogs>> = LazyLock::new(|| {
    Mutex::new(UploadedLogs {
        filenames: HashSet::new(),
    })
});
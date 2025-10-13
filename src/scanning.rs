use std::path::PathBuf;
use std::time::Duration;

use crate::logfile::LogFile;
use crate::settings::Settings;
use crate::state::{TimeFilter, STATE};

/// Checks if an auto-scan should be triggered (for "This session" mode)
pub fn check_auto_scan() {
    // Only auto-scan if we're in "This session" mode and on the log selection screen
    let current_filter = *STATE.selected_time_filter.lock().unwrap();
    let show_log_selection = *STATE.show_log_selection.lock().unwrap();
    let show_main_window = *STATE.show_main_window.lock().unwrap();

    // Only proceed if window is open AND we're on log selection screen
    if !show_main_window || !show_log_selection {
        return;
    }

    if current_filter == TimeFilter::SincePluginStart {
        let mut last_scan = STATE.last_auto_scan.lock().unwrap();
        let should_scan = last_scan
            .as_ref()
            .map_or(true, |t| t.elapsed() >= Duration::from_secs(20));

        if should_scan {
            *last_scan = Some(std::time::Instant::now());
            drop(last_scan);
            log::info!("Auto-scanning for new logs (This session mode)");
            scan_for_logs();
        }
    }
}

/// Updates the "last refreshed" display text
pub fn update_scan_display() {
    let last_scan = STATE.last_auto_scan.lock().unwrap();
    if let Some(scan_time) = *last_scan {
        let elapsed = scan_time.elapsed().as_secs();
        let display = if elapsed < 60 {
            format!(
                "Last refreshed: {} second{} ago",
                elapsed,
                if elapsed == 1 { "" } else { "s" }
            )
        } else {
            let minutes = elapsed / 60;
            format!(
                "Last refreshed: {} minute{} ago",
                minutes,
                if minutes == 1 { "" } else { "s" }
            )
        };
        *STATE.last_scan_display.lock().unwrap() = display;
    } else {
        *STATE.last_scan_display.lock().unwrap() = "Not yet refreshed".to_string();
    }
}

/// Recursively scans a directory for log files
fn scan_dir_recursive(
    dir: &std::path::Path,
    logs: &mut Vec<LogFile>,
    cutoff_time: Option<std::time::SystemTime>,
) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    scan_dir_recursive(&entry.path(), logs, cutoff_time);
                } else if metadata.is_file() {
                    if let Some(ext) = entry.path().extension() {
                        if ext == "zevtc" {
                            if let Ok(log) = LogFile::new(entry.path()) {
                                if cutoff_time.map_or(true, |cutoff| {
                                    let modified_time = std::time::SystemTime::UNIX_EPOCH
                                        + std::time::Duration::from_secs(log.modified);
                                    modified_time >= cutoff
                                }) {
                                    logs.push(log);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Scans for log files based on the current time filter
pub fn scan_for_logs() {
    // CRITICAL FIX: Capture settings and time filter BEFORE spawning the thread
    // This ensures we get the current settings state, not defaults
    let settings = Settings::get();
    let log_dir_string = settings.log_directory.clone();
    drop(settings);
    
    let time_filter = *STATE.selected_time_filter.lock().unwrap();
    
    std::thread::spawn(move || {
        log::info!("Starting background log scan");
        
        if log_dir_string.is_empty() {
            log::error!("Log directory is not configured");
            let mut logs = STATE.logs.lock().unwrap();
            logs.clear();
            return;
        }
        
        let log_dir = PathBuf::from(&log_dir_string);

        if !log_dir.exists() {
            log::error!("Log directory doesn't exist: {:?}", log_dir);
            let mut logs = STATE.logs.lock().unwrap();
            logs.clear();
            return;
        }

        let mut found_logs = Vec::new();

        let cutoff_time = match time_filter {
            TimeFilter::SincePluginStart => STATE.addon_load_time.lock().unwrap().map(|load_time| {
                std::time::SystemTime::now() - load_time.elapsed()
            }),
            TimeFilter::Last24Hours => Some(
                std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 60 * 60),
            ),
            TimeFilter::Last48Hours => Some(
                std::time::SystemTime::now() - std::time::Duration::from_secs(48 * 60 * 60),
            ),
            TimeFilter::Last72Hours => Some(
                std::time::SystemTime::now() - std::time::Duration::from_secs(72 * 60 * 60),
            ),
            TimeFilter::AllLogs => None,
        };

        scan_dir_recursive(&log_dir, &mut found_logs, cutoff_time);
        found_logs.sort_by(|a, b| b.modified.cmp(&a.modified));

        let filter_name = match time_filter {
            TimeFilter::SincePluginStart => "since plugin start",
            TimeFilter::Last24Hours => "24-hour",
            TimeFilter::Last48Hours => "48-hour",
            TimeFilter::Last72Hours => "72-hour",
            TimeFilter::AllLogs => "all logs",
        };

        let mut logs = STATE.logs.lock().unwrap();
        *logs = found_logs;
        log::info!("Found {} log files ({} filter)", logs.len(), filter_name);
    });
}
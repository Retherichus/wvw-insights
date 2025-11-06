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
                            // OPTIMIZATION: Check time filter BEFORE parsing
                            // This uses cheap filesystem metadata instead of expensive EVTC parsing
                            if let Some(cutoff) = cutoff_time {
                                if let Ok(modified) = metadata.modified() {
                                    if modified < cutoff {
                                        continue; // Skip - file too old, don't even parse it
                                    }
                                }
                            }
                            
                            // File is recent enough, now parse it to determine map type
                            if let Ok(log) = LogFile::new_fast(entry.path()) {
                                // Only include WvW logs (filters out PvE/Unknown)
                                if log.map_type.is_wvw() {
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
    // Set scanning flag to true at the start
    *STATE.scan_in_progress.lock().unwrap() = true;
    
    // Increment scan ID to invalidate any in-progress scans
    let scan_id = {
        let mut id = STATE.current_scan_id.lock().unwrap();
        *id += 1;
        *id
    };
    
    // Capture settings and time filter BEFORE spawning the thread
    let settings = Settings::get();
    let log_dir_string = settings.log_directory.clone();
    drop(settings);
    
    let time_filter = *STATE.selected_time_filter.lock().unwrap();
    
    std::thread::spawn(move || {
        log::info!("Starting background log scan (ID: {})", scan_id);
        
        if log_dir_string.is_empty() {
            log::error!("Log directory is not configured");
            let mut logs = STATE.logs.lock().unwrap();
            logs.clear();
            *STATE.scan_in_progress.lock().unwrap() = false;  // NEW: Clear scanning flag
            return;
        }
        
        let log_dir = PathBuf::from(&log_dir_string);

        if !log_dir.exists() {
            log::error!("Log directory doesn't exist: {:?}", log_dir);
            let mut logs = STATE.logs.lock().unwrap();
            logs.clear();
            *STATE.scan_in_progress.lock().unwrap() = false;  // NEW: Clear scanning flag
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
        };

        scan_dir_recursive(&log_dir, &mut found_logs, cutoff_time);
        found_logs.sort_by(|a, b| b.modified.cmp(&a.modified));

        // CHECK: Is this scan still the current one?
        let current_id = *STATE.current_scan_id.lock().unwrap();
        if scan_id != current_id {
            log::info!("Scan {} discarded (outdated, current is {})", scan_id, current_id);
            // Don't clear scanning flag here - a newer scan is running
            return;
        }

        let filter_name = match time_filter {
            TimeFilter::SincePluginStart => "since plugin start",
            TimeFilter::Last24Hours => "24-hour",
            TimeFilter::Last48Hours => "48-hour",
            TimeFilter::Last72Hours => "72-hour",
        };

        let mut logs = STATE.logs.lock().unwrap();
        // Preserve existing selections by filename
        let selections: std::collections::HashMap<String, bool> = logs
            .iter()
            .map(|log| (log.filename.clone(), log.selected))
            .collect();

        // Apply preserved selections to new logs
        for log in found_logs.iter_mut() {
            if let Some(&was_selected) = selections.get(&log.filename) {
                log.selected = was_selected;
            }
        }

        *logs = found_logs;
        log::info!("Scan {} completed: Found {} log files ({} filter)", scan_id, logs.len(), filter_name);
        
        // NEW: Clear scanning flag when scan is complete and current
        *STATE.scan_in_progress.lock().unwrap() = false;
    });
}
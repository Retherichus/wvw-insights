use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use winapi::shared::minwindef::TRUE;
use winapi::um::shellapi::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT, SHFILEOPSTRUCTW, SHFileOperationW,
};

use crate::settings::Settings;
use crate::state::STATE;

/// Checks if auto-cleanup should run on plugin load and executes it if enabled
pub fn check_auto_cleanup_on_load() {
    let settings = Settings::get();
    let enabled = settings.auto_cleanup_enabled;
    let days = settings.auto_cleanup_days;
    let log_dir = settings.log_directory.clone();
    drop(settings);

    if !enabled {
        return;
    }

    // Check if already done this session
    let mut done = STATE.auto_cleanup_done.lock().unwrap();
    if *done {
        return;
    }
    *done = true;
    drop(done);

    log::info!(
        "Auto-cleanup enabled, running cleanup for logs older than {} days",
        days
    );

    std::thread::spawn(move || match cleanup_old_logs(&log_dir, days) {
        Ok((files, bytes)) => {
            let mb = bytes as f64 / 1024.0 / 1024.0;
            log::info!(
                "Auto-cleanup complete: {} files ({:.2} MB) moved to Recycle Bin",
                files,
                mb
            );
        }
        Err(e) => {
            log::warn!("Auto-cleanup failed: {}", e);
        }
    });
}

/// Moves old log files to the Recycle Bin
pub fn cleanup_old_logs(log_directory: &str, days_old: u32) -> Result<(usize, u64), String> {
    if log_directory.is_empty() {
        return Err("No log directory configured".to_string());
    }

    let log_dir = PathBuf::from(log_directory);

    if !log_dir.exists() {
        return Err("Log directory does not exist".to_string());
    }

    let log_dir = match log_dir.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            return Err(format!("Invalid directory path: {}", e));
        }
    };

    let log_dir_str = log_dir.to_string_lossy().to_lowercase();
    let is_root = log_dir_str.ends_with(":\\") || log_dir_str.ends_with(":/");

    if is_root
        || log_dir_str.contains("\\windows\\")
        || log_dir_str.contains("\\program files")
    {
        return Err("Cannot clean system directories or drive roots".to_string());
    }

    let cutoff_time = std::time::SystemTime::now()
        - std::time::Duration::from_secs(days_old as u64 * 24 * 60 * 60);

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let temp_folder_name = format!("WvW_Insights_Cleanup_{}", timestamp);
    let temp_folder_path = log_dir.join(&temp_folder_name);

    if let Err(e) = std::fs::create_dir(&temp_folder_path) {
        return Err(format!("Failed to create temporary folder: {}", e));
    }

    let mut files_to_move = Vec::new();
    let mut total_size = 0u64;

    collect_old_logs_recursive(
        &log_dir,
        cutoff_time,
        &mut files_to_move,
        &mut total_size,
        &temp_folder_path,
    )
    .map_err(|e| format!("Failed to scan directory: {}", e))?;

    if files_to_move.is_empty() {
        let _ = std::fs::remove_dir(&temp_folder_path);
        return Ok((0, 0));
    }

    let mut moved_count = 0;
    let mut moved_size = 0u64;

    for file in files_to_move.iter() {
        let file_name = match file.file_name() {
            Some(name) => name,
            None => continue,
        };

        let mut dest_path = temp_folder_path.join(file_name);
        let mut counter = 1;
        while dest_path.exists() {
            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file");
            let ext = file
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("zevtc");
            dest_path = temp_folder_path.join(format!("{}_{}.{}", stem, counter, ext));
            counter += 1;
        }

        let move_result = std::fs::rename(file, &dest_path).or_else(|_| {
            std::fs::copy(file, &dest_path).and_then(|_| {
                std::fs::remove_file(file)?;
                Ok(())
            })
        });

        if move_result.is_ok() {
            if let Ok(metadata) = std::fs::metadata(&dest_path) {
                moved_size += metadata.len();
            }
            moved_count += 1;
        }
    }

    if moved_count == 0 {
        let _ = std::fs::remove_dir(&temp_folder_path);
        return Err("Failed to move any files".to_string());
    }

    log::info!(
        "Successfully moved {} files into temporary folder",
        moved_count
    );

    // Check if temp folder actually exists before attempting recycle
    if !temp_folder_path.exists() {
        log::error!(
            "Temp folder doesn't exist after moving files: {:?}",
            temp_folder_path
        );
        return Err("Temp folder disappeared after moving files".to_string());
    }

    log::info!(
        "Temp folder exists, attempting to send to Recycle Bin: {:?}",
        temp_folder_path
    );

    // CRITICAL FIX: Strip the \\?\ prefix that canonicalize adds
    // SHFileOperationW doesn't support the \\?\ prefix
    let path_for_shell = temp_folder_path.to_string_lossy();
    let path_for_shell = if path_for_shell.starts_with(r"\\?\") {
        &path_for_shell[4..] // Remove \\?\ prefix
    } else {
        &path_for_shell
    };

    log::info!(
        "Path for shell operation (without \\\\?\\ prefix): {}",
        path_for_shell
    );

    // Convert to wide string with double null terminator
    let path_buffer: Vec<u16> = std::ffi::OsStr::new(path_for_shell)
        .encode_wide()
        .chain(std::iter::once(0))
        .chain(std::iter::once(0))
        .collect();

    log::info!(
        "Path buffer length: {}, last 4 values: {:?}",
        path_buffer.len(),
        &path_buffer[path_buffer.len().saturating_sub(4)..]
    );

    let mut file_op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE as u32,
        pFrom: path_buffer.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };

    log::info!("Calling SHFileOperationW...");
    let result = unsafe { SHFileOperationW(&mut file_op) };
    log::info!(
        "SHFileOperationW returned: {}, aborted: {}",
        result,
        file_op.fAnyOperationsAborted
    );

    // Check if folder still exists after the operation
    let folder_still_exists = temp_folder_path.exists();
    log::info!(
        "Temp folder exists after operation: {}",
        folder_still_exists
    );

    if result == 0 && file_op.fAnyOperationsAborted != TRUE {
        log::info!(
            "Cleanup: {} files ({:.2} MB) moved to Recycle Bin",
            moved_count,
            moved_size as f64 / 1024.0 / 1024.0
        );
        Ok((moved_count, moved_size))
    } else {
        log::error!(
            "SHFileOperationW failed with code: {}, aborted: {}",
            result,
            file_op.fAnyOperationsAborted
        );

        // DON'T delete the folder - it contains user's files!
        if folder_still_exists {
            log::warn!("Temp folder still exists at: {:?}", temp_folder_path);
            log::warn!("User can manually move this folder to Recycle Bin");
        } else {
            log::error!("WARNING: Temp folder disappeared but wasn't sent to Recycle Bin!");
        }

        Err(format!(
            "Failed to move folder to Recycle Bin (error: {}, folder exists: {})",
            result, folder_still_exists
        ))
    }
}

/// Recursively collects old log files from a directory
fn collect_old_logs_recursive(
    dir: &std::path::Path,
    cutoff: std::time::SystemTime,
    files: &mut Vec<PathBuf>,
    size: &mut u64,
    exclude_folder: &std::path::Path,
) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            return Err(format!("Failed to read directory: {}", e));
        }
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();

        // Skip the temp folder we just created
        if entry_path == exclude_folder {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.is_dir() {
            // Skip ANY folder with "WvW_Insights_Cleanup" in its name
            if let Some(dir_name) = entry_path.file_name() {
                if dir_name
                    .to_string_lossy()
                    .contains("WvW_Insights_Cleanup")
                {
                    log::info!("Skipping cleanup temp folder: {:?}", entry_path);
                    continue;
                }
            }
            collect_old_logs_recursive(&entry_path, cutoff, files, size, exclude_folder)?;
        } else if metadata.is_file() {
            if let Some(ext) = entry_path.extension() {
                if ext == "zevtc" {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            files.push(entry_path);
                            *size += metadata.len();
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
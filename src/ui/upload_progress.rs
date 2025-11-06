use nexus::imgui::{ChildWindow, ProgressBar, Ui};

use crate::settings::Settings;
use crate::state::{ProcessingState, STATE};

/// File processing status for individual files
#[derive(Clone, Debug)]
enum FileStatus {
    Pending,
    Processing,
    Complete,
}

/// Renders the upload progress screen with individual file tracking
pub fn render_upload_progress(ui: &Ui) {
    let state = *STATE.processing_state.lock().unwrap();
    
    // Show total files in session at the top
    let total_files = STATE.uploaded_files.lock().unwrap().len();
    ui.text(format!("Upload Progress - {} file(s) in session", total_files));
    ui.separator();

    ChildWindow::new("UploadStatus")
        .size([0.0, 300.0])
        .build(ui, || {
            // During uploading, show the logs being uploaded with their status
            if state == ProcessingState::Uploading {
                let logs = STATE.logs.lock().unwrap();
                let has_selected = logs.iter().any(|l| l.selected);
                
                if has_selected {
                    for log in logs.iter() {
                        if log.selected {
                            ui.text(format!("{}: {}", log.filename, log.status));
                        }
                    }
                } else {
                    ui.text_colored([0.7, 0.7, 0.7, 1.0], "No files selected for upload");
                }
            } else if state == ProcessingState::Processing {
                // Show file-by-file progress during processing
                render_file_processing_status(ui);
            } else {
                // Show all files in the current session (Idle/Complete/Failed states)
                let uploaded_files = STATE.uploaded_files.lock().unwrap();
                
                if uploaded_files.is_empty() {
                    ui.text_colored([0.7, 0.7, 0.7, 1.0], "No files in session");
                } else {
                    for file in uploaded_files.iter() {
                        let status_text = if state == ProcessingState::Complete {
                            "[OK] Processed"
                        } else {
                            "Uploaded"
                        };
                        
                        let status_color = if state == ProcessingState::Complete {
                            [0.0, 1.0, 0.0, 1.0]
                        } else {
                            [0.7, 0.9, 1.0, 1.0]
                        };
                        
                        ui.text(&file.filename);
                        ui.same_line();
                        ui.text_colored(status_color, &format!("- {}", status_text));
                    }
                }
            }
        });

    let state = *STATE.processing_state.lock().unwrap();

    ui.separator();

    match state {
        ProcessingState::Uploading => {
            ui.text("Uploading files...");
            ui.spacing();

            if ui.button("Cancel Upload") {
                std::thread::spawn(|| {
                    log::info!("User cancelled upload");
                    reset_upload_state();
                    *STATE.show_log_selection.lock().unwrap() = false;
                    *STATE.show_token_input.lock().unwrap() = true;
                });
            }
        }
            ProcessingState::Idle => {
                let logs = STATE.logs.lock().unwrap();
                let selected_logs: Vec<_> = logs.iter().filter(|l| l.selected).collect();
                let total = selected_logs.len();
                let uploaded = selected_logs
                    .iter()
                    .filter(|l| l.uploaded || l.status.starts_with("Failed"))
                    .count();
                drop(logs);

                if uploaded >= total && total > 0 {
                    ui.text_colored([0.0, 1.0, 0.0, 1.0], "All files uploaded successfully!");
                    ui.spacing();

                    if ui.button("Start Processing") {
                        *STATE.processing_state.lock().unwrap() = ProcessingState::Processing;

                        std::thread::spawn(|| {
                            let settings = Settings::get();
                            let api_endpoint = settings.api_endpoint.clone();
                            let history_token = settings.history_token.clone();
                            let guild_name = settings.guild_name.clone();
                            let enable_legacy_parser = settings.enable_legacy_parser;
                            let dps_report_token = settings.dps_report_token.clone(); // ADD THIS LINE
                            drop(settings);

                            let session_id = STATE.session_id.lock().unwrap().clone();
                            let ownership_token = STATE.ownership_token.lock().unwrap().clone();

                            log::info!("Starting processing with guild name: '{}', legacy parser: {}", guild_name, enable_legacy_parser);
                            match crate::upload::start_processing(
                                &api_endpoint,
                                &session_id,
                                &history_token,
                                &ownership_token,
                                &guild_name,
                                enable_legacy_parser,
                                &dps_report_token,
                            ) {
                            Ok(server_message) => {
                                log::info!("Processing started successfully: {}", server_message);
                                *STATE.last_status_check.lock().unwrap() =
                                    Some(std::time::Instant::now());
                            }
                            Err(e) => {
                                log::error!("Failed to start processing: {}", e);
                                *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
                                *STATE.report_urls.lock().unwrap() = vec![format!("Server error: {}", e)];
                            }
                        }
                    });
                }

                ui.same_line();

                if ui.button("Cancel") {
                    std::thread::spawn(|| {
                        log::info!("User cancelled before processing");
                        reset_upload_state();
                        *STATE.show_log_selection.lock().unwrap() = false;
                        *STATE.show_token_input.lock().unwrap() = true;
                    });
                }
            } else {
                ui.text("Uploading files...");
            }
        }
        ProcessingState::Processing => {
            let progress = *STATE.processing_progress.lock().unwrap();
            let phase = STATE.processing_phase.lock().unwrap().clone();

            // Check if we're in queued state (progress will be 0 and phase will contain "Queued")
            if progress == 0.0 && phase.contains("Queued") {
                ui.text_colored([1.0, 1.0, 0.0, 1.0], &phase);
                ui.spacing();
                ui.text_colored([0.7, 0.9, 1.0, 1.0], "Your session is waiting in the processing queue...");
                ui.spacing();
                ui.text_colored([0.7, 0.7, 0.7, 1.0], "Processing will begin automatically when a slot becomes available.");
            } else {
                if !phase.is_empty() {
                    ui.text(&phase);
                } else {
                    ui.text("Processing logs on server...");
                }

                ui.spacing();

                // Progress bar
                let progress_fraction = progress / 100.0;
                ui.text(format!("Progress: {:.0}%", progress));
                ProgressBar::new(progress_fraction).size([0.0, 0.0]).build(ui);

                // Show time estimate countdown if available
                let time_estimate = *STATE.processing_time_estimate.lock().unwrap();
                let timer_start = *STATE.processing_time_estimate_start.lock().unwrap();
                
                if let (Some(estimate_seconds), Some(start_time)) = (time_estimate, timer_start) {
                    ui.spacing();
                    
                    let elapsed = start_time.elapsed().as_secs() as u32;
                    
                    if elapsed < estimate_seconds {
                        // Countdown mode - still within estimate
                        let remaining = estimate_seconds - elapsed;
                        
                        if remaining < 60 {
                            ui.text_colored([0.7, 0.9, 1.0, 1.0], &format!("Estimated: ~{} seconds remaining", remaining));
                        } else {
                            let minutes = remaining / 60;
                            let seconds = remaining % 60;
                            if seconds > 0 {
                                ui.text_colored([0.7, 0.9, 1.0, 1.0], &format!("Estimated: ~{} min {} sec remaining", minutes, seconds));
                            } else {
                                ui.text_colored([0.7, 0.9, 1.0, 1.0], &format!("Estimated: ~{} minutes remaining", minutes));
                            }
                        }
                    } else {
                        // Overdue mode - exceeded estimate
                        let overdue = elapsed - estimate_seconds;
                        
                        if overdue < 60 {
                            ui.text_colored([1.0, 0.8, 0.2, 1.0], &format!("Overdue by {} seconds (still processing...)", overdue));
                        } else {
                            let minutes = overdue / 60;
                            let seconds = overdue % 60;
                            if seconds > 0 {
                                ui.text_colored([1.0, 0.8, 0.2, 1.0], &format!("Overdue by {} min {} sec (still processing...)", minutes, seconds));
                            } else {
                                ui.text_colored([1.0, 0.8, 0.2, 1.0], &format!("Overdue by {} minutes (still processing...)", minutes));
                            }
                        }
                    }
                }

                ui.spacing();
                ui.text_colored([1.0, 1.0, 0.0, 1.0], "This may take several minutes...");
            }

            ui.spacing();
            ui.separator();
            ui.spacing();

            if ui.button("Cancel Processing") {
                std::thread::spawn(|| {
                    log::info!("User cancelled processing");
                    reset_upload_state();
                    *STATE.show_log_selection.lock().unwrap() = false;
                    *STATE.show_token_input.lock().unwrap() = true;
                });
            }
        }
        ProcessingState::Complete => {
            ui.text_colored([0.0, 1.0, 0.0, 1.0], "Processing complete!");
            
            let report_urls = STATE.report_urls.lock().unwrap();
            if !report_urls.is_empty() {
                ui.spacing();
                ui.text("Report URLs:");
                
                for url in report_urls.iter() {
                    let label = if url.contains("Legacy") || url.to_lowercase().contains("legacy") {
                        "Legacy Report:"
                    } else {
                        "Report:"
                    };
                    ui.text_colored([0.0, 1.0, 1.0, 1.0], &format!("{} {}", label, url));
                }
            } else {
                ui.spacing();
                ui.text_colored([1.0, 1.0, 0.0, 1.0], "No report URLs available");
            }

            ui.spacing();
            if ui.button("Back to Log Selection") {
                std::thread::spawn(|| {
                    log::info!("Back to Log Selection clicked - spawning reset");
                    reset_upload_state();
                    log::info!("Reset complete");
                });
            }
        }
            ProcessingState::Failed => {
                let report_urls = STATE.report_urls.lock().unwrap();
                let error_message = report_urls.first().cloned().unwrap_or_default();
                drop(report_urls);

                ui.text_colored([1.0, 0.0, 0.0, 1.0], "Processing failed!");
                ui.spacing();

                if !error_message.is_empty() {
                    ui.text("Server response:");
                    ui.text_colored([1.0, 0.5, 0.5, 1.0], &error_message);
                    ui.spacing();
                }

                if ui.button("Retry Processing") {
                    *STATE.processing_state.lock().unwrap() = ProcessingState::Processing;
                    STATE.report_urls.lock().unwrap().clear();

                    std::thread::spawn(|| {
                        let settings = Settings::get();
                        let api_endpoint = settings.api_endpoint.clone();
                        let history_token = settings.history_token.clone();
                        let guild_name = settings.guild_name.clone();
                        let enable_legacy_parser = settings.enable_legacy_parser;
                        let dps_report_token = settings.dps_report_token.clone(); // ADD THIS LINE
                        drop(settings);

                        let session_id = STATE.session_id.lock().unwrap().clone();
                        let ownership_token = STATE.ownership_token.lock().unwrap().clone();

                        log::info!("Retrying processing with guild name: '{}', legacy parser: {}", guild_name, enable_legacy_parser);
                        match crate::upload::start_processing(
                            &api_endpoint,
                            &session_id,
                            &history_token,
                            &ownership_token,
                            &guild_name,
                            enable_legacy_parser,
                            &dps_report_token,
                        ) {
                        Ok(server_message) => {
                            log::info!("Processing started successfully: {}", server_message);
                            *STATE.last_status_check.lock().unwrap() =
                                Some(std::time::Instant::now());
                        }
                        Err(e) => {
                            log::error!("Failed to start processing: {}", e);
                            *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
                            *STATE.report_urls.lock().unwrap() = vec![format!("Server error: {}", e)];
                        }
                    }
                });
            }

            ui.same_line();

            if ui.button("Back to Log Selection") {
                std::thread::spawn(|| {
                    log::info!("Back to Log Selection clicked - spawning reset");
                    reset_upload_state();
                    log::info!("Reset complete");
                });
            }
        }
    }
}

/// Renders file-by-file processing status during the Processing state
fn render_file_processing_status(ui: &Ui) {
    let uploaded_files = STATE.uploaded_files.lock().unwrap();
    let phase = STATE.processing_phase.lock().unwrap();
    let progress = *STATE.processing_progress.lock().unwrap();
    
    // Extract file progress from the phase string
    // Format: "Processing logs with Elite Insights (3/4)"
    let (current_file, total_files) = extract_file_progress(&phase);
    
    if uploaded_files.is_empty() {
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "No files in session");
        return;
    }
    
    let total_uploaded = uploaded_files.len();
    
    // Determine status for each file
    for (index, file) in uploaded_files.iter().enumerate() {
        let file_number = index + 1;
        
        let status = if current_file > 0 && total_files > 0 {
            // We have file tracking info from Elite Insights
            if file_number < current_file {
                FileStatus::Complete
            } else if file_number == current_file {
                FileStatus::Processing
            } else {
                FileStatus::Pending
            }
        } else if progress >= 25.0 {
            // Elite Insights phase is complete (progress >= 25%), mark all files as complete
            FileStatus::Complete
        } else {
            // No file tracking yet, just mark first file as processing
            if index == 0 {
                FileStatus::Processing
            } else {
                FileStatus::Pending
            }
        };
        
        render_file_item(ui, file, &status, file_number, total_uploaded);
    }
}

/// Renders a single file item with its processing status
fn render_file_item(ui: &Ui, file: &crate::upload_review::UploadedFileInfo, status: &FileStatus, file_num: usize, total: usize) {
    let (icon, color) = match status {
        FileStatus::Complete => ("[OK]", [0.0, 1.0, 0.0, 1.0]),
        FileStatus::Processing => ("[>>]", [1.0, 0.8, 0.2, 1.0]),
        FileStatus::Pending => ("[ ]", [0.5, 0.5, 0.5, 1.0]),
    };
    
    let status_text = match status {
        FileStatus::Complete => "Complete".to_string(),
        FileStatus::Processing => format!("Processing ({}/{})", file_num, total),
        FileStatus::Pending => "Pending".to_string(),
    };
    
    // Icon
    ui.text_colored(color, icon);
    ui.same_line();
    
    // Filename
    ui.text(&file.filename);
    ui.same_line();
    
    // Status
    ui.text_colored(color, &format!("- {}", status_text));
}

/// Extracts current file and total files from phase message
/// Returns (current, total) or (0, 0) if not found
fn extract_file_progress(phase: &str) -> (usize, usize) {
    // Look for pattern like "Processing logs with Elite Insights (3/4)"
    if let Some(start) = phase.rfind('(') {
        if let Some(end) = phase.rfind(')') {
            if end > start {
                let progress_str = &phase[start + 1..end];
                if let Some(slash_pos) = progress_str.find('/') {
                    let current_str = &progress_str[..slash_pos];
                    let total_str = &progress_str[slash_pos + 1..];
                    
                    if let (Ok(current), Ok(total)) = (current_str.parse::<usize>(), total_str.parse::<usize>()) {
                        return (current, total);
                    }
                }
            }
        }
    }
    
    (0, 0)
}

/// Resets the upload state to allow starting a new upload
pub fn reset_upload_state() {
    log::info!("reset_upload_state: Starting");

    log::info!("reset_upload_state: Resetting show_upload_progress");
    *STATE.show_upload_progress.lock().unwrap() = false;

    log::info!("reset_upload_state: Resetting show_results");
    *STATE.show_results.lock().unwrap() = false;

    log::info!("reset_upload_state: Resetting show_upload_review");
    *STATE.show_upload_review.lock().unwrap() = false;

    log::info!("reset_upload_state: Resetting processing_state");
    *STATE.processing_state.lock().unwrap() = ProcessingState::Idle;

    log::info!("reset_upload_state: Clearing report_urls");
    STATE.report_urls.lock().unwrap().clear();

    log::info!("reset_upload_state: Clearing session_id");
    STATE.session_id.lock().unwrap().clear();

    log::info!("reset_upload_state: Clearing ownership_token");
    STATE.ownership_token.lock().unwrap().clear();

    log::info!("reset_upload_state: Clearing uploaded_files");
    STATE.uploaded_files.lock().unwrap().clear();

    log::info!("reset_upload_state: Resetting last_status_check");
    *STATE.last_status_check.lock().unwrap() = None;

    log::info!("reset_upload_state: Resetting processing_progress");
    *STATE.processing_progress.lock().unwrap() = 0.0;

    log::info!("reset_upload_state: Clearing processing_phase");
    STATE.processing_phase.lock().unwrap().clear();
    
    log::info!("reset_upload_state: Clearing processing_time_estimate");
    *STATE.processing_time_estimate.lock().unwrap() = None;
    
    log::info!("reset_upload_state: Clearing processing_time_estimate_start");
    *STATE.processing_time_estimate_start.lock().unwrap() = None;

    log::info!("reset_upload_state: Locking logs for reset");
    let mut logs = STATE.logs.lock().unwrap();
    log::info!(
        "reset_upload_state: Got logs lock, resetting {} logs",
        logs.len()
    );
    for log in logs.iter_mut() {
        log.selected = false;
        log.uploaded = false;
        log.status = "Ready".to_string();
    }
    drop(logs);
    log::info!("reset_upload_state: Logs reset complete");

    log::info!("reset_upload_state: Setting show_log_selection to true");
    *STATE.show_log_selection.lock().unwrap() = true;

    log::info!("reset_upload_state: Complete");
}
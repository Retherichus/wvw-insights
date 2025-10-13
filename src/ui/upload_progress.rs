use nexus::imgui::{ChildWindow, ProgressBar, Ui};

use crate::settings::Settings;
use crate::state::{ProcessingState, STATE};

/// Renders the upload progress screen
pub fn render_upload_progress(ui: &Ui) {
    ui.text("Upload Progress");
    ui.separator();

    ChildWindow::new("UploadStatus")
        .size([0.0, 300.0])
        .build(ui, || {
            let logs = STATE.logs.lock().unwrap();
            for log in logs.iter() {
                if log.selected {
                    ui.text(format!("{}: {}", log.filename, log.status));
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
                ui.open_popup("cancel_upload_confirmation");
            }

            ui.popup_modal("cancel_upload_confirmation")
                .always_auto_resize(true)
                .build(ui, || {
                    ui.text("Are you sure you want to cancel this job?");
                    ui.spacing();
                    ui.text_colored([1.0, 1.0, 0.0, 1.0], "The upload will be abandoned.");
                    ui.spacing();

                    if ui.button("Yes, Cancel") {
                        ui.close_current_popup();
                        std::thread::spawn(|| {
                            log::info!("User cancelled upload");
                            reset_upload_state();
                            *STATE.show_log_selection.lock().unwrap() = false;
                            *STATE.show_token_input.lock().unwrap() = true;
                        });
                    }

                    ui.same_line();

                    if ui.button("No, Continue") {
                        ui.close_current_popup();
                    }
                });
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
                        drop(settings);

                        let session_id = STATE.session_id.lock().unwrap().clone();
                        let ownership_token = STATE.ownership_token.lock().unwrap().clone();

                        log::info!("Starting processing");
                        match crate::upload::start_processing(
                            &api_endpoint,
                            &session_id,
                            &history_token,
                            &ownership_token,
                        ) {
                            Ok(server_message) => {
                                log::info!("Processing started successfully: {}", server_message);
                                *STATE.last_status_check.lock().unwrap() =
                                    Some(std::time::Instant::now());
                            }
                            Err(e) => {
                                log::error!("Failed to start processing: {}", e);
                                *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
                                *STATE.report_url.lock().unwrap() = format!("Server error: {}", e);
                            }
                        }
                    });
                }

                ui.same_line();

                if ui.button("Cancel") {
                    ui.open_popup("cancel_before_processing");
                }

                ui.popup_modal("cancel_before_processing")
                    .always_auto_resize(true)
                    .build(ui, || {
                        ui.text("Are you sure you want to cancel this job?");
                        ui.spacing();
                        ui.text_colored(
                            [1.0, 1.0, 0.0, 1.0],
                            "The uploaded files will be abandoned.",
                        );
                        ui.spacing();

                        if ui.button("Yes, Cancel") {
                            ui.close_current_popup();
                            std::thread::spawn(|| {
                                log::info!("User cancelled before processing");
                                reset_upload_state();
                                *STATE.show_log_selection.lock().unwrap() = false;
                                *STATE.show_token_input.lock().unwrap() = true;
                            });
                        }

                        ui.same_line();

                        if ui.button("No, Continue") {
                            ui.close_current_popup();
                        }
                    });
            } else {
                ui.text("Uploading files...");
            }
        }
        ProcessingState::Processing => {
            let progress = *STATE.processing_progress.lock().unwrap();
            let phase = STATE.processing_phase.lock().unwrap().clone();

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

            ui.spacing();
            ui.text_colored([1.0, 1.0, 0.0, 1.0], "This may take several minutes...");

            ui.spacing();
            ui.separator();
            ui.spacing();

            if ui.button("Cancel Processing") {
                ui.open_popup("cancel_processing");
            }

            ui.popup_modal("cancel_processing")
                .always_auto_resize(true)
                .build(ui, || {
                    ui.text("Are you sure you want to cancel this job?");
                    ui.spacing();
                    ui.text_colored(
                        [1.0, 1.0, 0.0, 1.0],
                        "The server will finish processing in the background,",
                    );
                    ui.text_colored(
                        [1.0, 1.0, 0.0, 1.0],
                        "but you won't be able to see the results.",
                    );
                    ui.spacing();

                    if ui.button("Yes, Cancel") {
                        ui.close_current_popup();
                        std::thread::spawn(|| {
                            log::info!("User cancelled processing");
                            reset_upload_state();
                            *STATE.show_log_selection.lock().unwrap() = false;
                            *STATE.show_token_input.lock().unwrap() = true;
                        });
                    }

                    ui.same_line();

                    if ui.button("No, Continue") {
                        ui.close_current_popup();
                    }
                });
        }
        ProcessingState::Complete => {
            ui.text_colored([0.0, 1.0, 0.0, 1.0], "Processing complete!");
        }
        ProcessingState::Failed => {
            let error_message = STATE.report_url.lock().unwrap().clone();
            drop(STATE.report_url.lock());

            ui.text_colored([1.0, 0.0, 0.0, 1.0], "Processing failed!");
            ui.spacing();

            if !error_message.is_empty() {
                ui.text("Server response:");
                ui.text_colored([1.0, 0.5, 0.5, 1.0], &error_message);
                ui.spacing();
            }

            if ui.button("Retry Processing") {
                *STATE.processing_state.lock().unwrap() = ProcessingState::Processing;
                STATE.report_url.lock().unwrap().clear();

                std::thread::spawn(|| {
                    let settings = Settings::get();
                    let api_endpoint = settings.api_endpoint.clone();
                    let history_token = settings.history_token.clone();
                    drop(settings);

                    let session_id = STATE.session_id.lock().unwrap().clone();
                    let ownership_token = STATE.ownership_token.lock().unwrap().clone();

                    log::info!("Retrying processing");
                    match crate::upload::start_processing(
                        &api_endpoint,
                        &session_id,
                        &history_token,
                        &ownership_token,
                    ) {
                        Ok(server_message) => {
                            log::info!("Processing started successfully: {}", server_message);
                            *STATE.last_status_check.lock().unwrap() =
                                Some(std::time::Instant::now());
                        }
                        Err(e) => {
                            log::error!("Failed to start processing: {}", e);
                            *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
                            *STATE.report_url.lock().unwrap() = format!("Server error: {}", e);
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

/// Resets the upload state to allow starting a new upload
pub fn reset_upload_state() {
    log::info!("reset_upload_state: Starting");

    log::info!("reset_upload_state: Resetting show_upload_progress");
    *STATE.show_upload_progress.lock().unwrap() = false;

    log::info!("reset_upload_state: Resetting show_results");
    *STATE.show_results.lock().unwrap() = false;

    log::info!("reset_upload_state: Resetting processing_state");
    *STATE.processing_state.lock().unwrap() = ProcessingState::Idle;

    log::info!("reset_upload_state: Clearing report_url");
    STATE.report_url.lock().unwrap().clear();

    log::info!("reset_upload_state: Clearing session_id");
    STATE.session_id.lock().unwrap().clear();

    log::info!("reset_upload_state: Clearing ownership_token");
    STATE.ownership_token.lock().unwrap().clear();

    log::info!("reset_upload_state: Resetting last_status_check");
    *STATE.last_status_check.lock().unwrap() = None;

    log::info!("reset_upload_state: Resetting processing_progress");
    *STATE.processing_progress.lock().unwrap() = 0.0;

    log::info!("reset_upload_state: Clearing processing_phase");
    STATE.processing_phase.lock().unwrap().clear();

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
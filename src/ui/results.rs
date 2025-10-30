use nexus::imgui::Ui;

use crate::scanning::scan_for_logs;
use crate::state::STATE;
use crate::ui::upload_progress::reset_upload_state;
use crate::uploaded_logs::UploadedLogs;
use crate::webhooks::{send_to_discord, WebhookSettings};

thread_local! {
    static REPORT_NAME_BUFFER: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
}

/// Renders the results screen after processing is complete
pub fn render_results(ui: &Ui) {
    ui.text("Processing Complete!");
    ui.spacing();

    let report_urls = STATE.report_urls.lock().unwrap();
    
    if report_urls.is_empty() {
        ui.text_colored([1.0, 1.0, 0.0, 1.0], "No report URLs available");
    } else {
        ui.text("Your reports are ready:");
        
        for url in report_urls.iter() {
            let label = if url.contains("Legacy") || url.to_lowercase().contains("legacy") {
                "Legacy Report:"
            } else {
                "Report:"
            };
            
            ui.text(label);
            ui.text_wrapped(url);
            
            // Use the URL itself as the unique ID for buttons
            let copy_id = format!("Copy URL##{}", url);
            if ui.button(&copy_id) {
                ui.set_clipboard_text(url);
            }
            
            ui.same_line();

            let open_id = format!("Open in Browser##{}", url);
            if ui.button(&open_id) {
                if let Err(e) = open::that_detached(url.as_str()) {
                    log::error!("Failed to open browser: {}", e);
                }
            }
            
            ui.spacing();
        }

        // Copy Both URLs button (only show if multiple reports)
        if report_urls.len() > 1 {
            if ui.button("Copy Both URLs") {
                let combined_urls = report_urls.join("\n-\n");
                ui.set_clipboard_text(&combined_urls);
            }
            ui.spacing();
        }

        // Send to Discord button
        if ui.button("Send to Discord") {
            *STATE.show_webhook_modal.lock().unwrap() = true;
            
            // Load remembered webhook if available
            let webhook_settings = WebhookSettings::get();
            if webhook_settings.remember_last_webhook && !webhook_settings.last_webhook_url.is_empty() {
                *STATE.webhook_url_input.lock().unwrap() = webhook_settings.last_webhook_url.clone();
                *STATE.webhook_remember.lock().unwrap() = true;
            } else {
                STATE.webhook_url_input.lock().unwrap().clear();
                *STATE.webhook_remember.lock().unwrap() = false;
            }
            drop(webhook_settings);
            
            // Initialize report name with default pattern
            REPORT_NAME_BUFFER.with(|buffer| {
                let current_date = chrono::Local::now().format("%d.%m.%y").to_string();
                *buffer.borrow_mut() = format!("WvW: {}", current_date);
            });
        }
    }

    drop(report_urls);

    ui.spacing();
    ui.separator();

    if ui.button("Upload More Logs") {
        log::info!("Upload More Logs button clicked");
        std::thread::spawn(|| {
            log::info!("Resetting upload state and clearing session");
            
            // Mark uploaded logs BEFORE resetting state
            mark_uploaded_logs();
            
            // Clear the session completely
            log::info!("Clearing session data");
            STATE.session_id.lock().unwrap().clear();
            STATE.ownership_token.lock().unwrap().clear();
            STATE.uploaded_files.lock().unwrap().clear();
            
            // Reset states
            reset_upload_state();
            
            log::info!("State reset complete, starting log scan");
            scan_for_logs();
            log::info!("Log scan complete");
        });
    }

    ui.same_line();

    if ui.button("Back to Start") {
        std::thread::spawn(|| {
            log::info!("Back to Start button clicked");
            
            // Mark uploaded logs BEFORE resetting state
            mark_uploaded_logs();
            
            // Clear the session completely
            log::info!("Clearing session data for back to start");
            STATE.session_id.lock().unwrap().clear();
            STATE.ownership_token.lock().unwrap().clear();
            STATE.uploaded_files.lock().unwrap().clear();
            
            // Reset all states
            reset_upload_state();
            
            // Go to token input instead of log selection
            *STATE.show_log_selection.lock().unwrap() = false;
            *STATE.show_token_input.lock().unwrap() = true;
            
            log::info!("Back to start complete");
        });
    }

    // Render webhook modal if open
    let show_modal = *STATE.show_webhook_modal.lock().unwrap();
    if show_modal {
        render_webhook_modal(ui);
    }
}


/// Renders the Discord webhook modal
fn render_webhook_modal(ui: &Ui) {
    ui.open_popup("Send to Discord");
    
    ui.popup_modal("Send to Discord")
        .always_auto_resize(true)
        .build(ui, || {
            // Show status message if active - check and drop lock before rendering buttons
            let should_show_status = {
                let status_until = STATE.webhook_status_until.lock().unwrap();
                if let Some(until) = *status_until {
                    std::time::Instant::now() < until
                } else {
                    false
                }
            };
            
            if should_show_status {
                let message = STATE.webhook_status_message.lock().unwrap().clone();
                let is_error = *STATE.webhook_status_is_error.lock().unwrap();
                
                let color = if is_error {
                    [1.0, 0.5, 0.0, 1.0]
                } else {
                    [0.0, 1.0, 0.0, 1.0]
                };
                
                ui.text_colored(color, &message);
                ui.spacing();
            }

            // Saved webhooks section
            ui.text("Saved Webhooks:");
            
            let webhook_settings = WebhookSettings::get();
            let webhooks = webhook_settings.get_webhooks_sorted();
            
            if webhooks.is_empty() {
                ui.text_colored([0.7, 0.7, 0.7, 1.0], "No saved webhooks. Add one in Settings.");
            } else {
                for webhook in webhooks.iter() {
                    let button_label = format!("{}##{}", webhook.name, webhook.name);
                    if ui.button(&button_label) {
                        *STATE.webhook_url_input.lock().unwrap() = webhook.url.clone();
                        *STATE.webhook_selected_name.lock().unwrap() = webhook.name.clone();
                    }
                }
            }
            drop(webhook_settings);
            
            ui.spacing();
            ui.separator();
            ui.spacing();

            // Webhook URL input
            ui.text("Webhook URL:");
            let mut url = STATE.webhook_url_input.lock().unwrap();
            ui.input_text("##webhook_url", &mut *url)
                .hint("https://discord.com/api/webhooks/...")
                .build();
            drop(url);

            let mut remember = *STATE.webhook_remember.lock().unwrap();
            if ui.checkbox("Remember this webhook", &mut remember) {
                *STATE.webhook_remember.lock().unwrap() = remember;
            }

            ui.spacing();
            ui.separator();
            ui.spacing();

            // Report name input (only for main report)
            ui.text("Report Name:");
            REPORT_NAME_BUFFER.with(|buffer| {
                let mut name = buffer.borrow_mut();
                ui.input_text("##report_name", &mut *name)
                    .hint("WvW: DD.MM.YY")
                    .build();
            });
            
            ui.text_colored([0.7, 0.7, 0.7, 1.0], "Tip: Use (*DATE) to auto-fill with current date");
            ui.text_colored([0.7, 0.7, 0.7, 1.0], "Legacy reports will always be labeled 'Legacy Report'");

            ui.spacing();
            ui.separator();
            ui.spacing();

            // Preview section - show all reports
            let report_urls = STATE.report_urls.lock().unwrap();
            let num_reports = report_urls.len();
            
            // Dynamic preview header based on number of reports
            let preview_text = if num_reports > 1 {
                "Preview (All reports will be sent):"
            } else {
                "Preview:"
            };
            ui.text(preview_text);
            
            let report_name = REPORT_NAME_BUFFER.with(|buffer| {
                let name = buffer.borrow().clone();
                // Replace (*DATE) with current date
                let current_date = chrono::Local::now().format("%d.%m.%y").to_string();
                name.replace("(*DATE)", &current_date)
            });
            
            // Show all reports in preview
            ui.indent();
            for url in report_urls.iter() {
                let is_legacy = url.contains("Legacy") || url.to_lowercase().contains("legacy");
                
                if is_legacy {
                    ui.text_colored([0.3, 0.7, 1.0, 1.0], "Legacy Report:");
                    ui.text_colored([0.5, 0.5, 0.5, 1.0], &format!("Link: {}", url));
                } else {
                    ui.text_colored([0.3, 0.7, 1.0, 1.0], &report_name);
                    ui.text_colored([0.5, 0.5, 0.5, 1.0], &format!("Link: {}", url));
                }
                ui.spacing();
            }
            ui.unindent();
            
            drop(report_urls);
            
            ui.spacing();
            ui.separator();
            ui.spacing();

            // Send button
            let is_sending = *STATE.webhook_sending.lock().unwrap();
            
            if is_sending {
                ui.text("Sending...");
            } else {
                if ui.button("Send now!") {
                    let webhook_url = STATE.webhook_url_input.lock().unwrap().clone();
                    let remember = *STATE.webhook_remember.lock().unwrap();
                    
                    // Validate URL on main thread
                    if webhook_url.trim().is_empty() {
                        show_webhook_message("Please enter a webhook URL", true);
                    } else if !webhook_url.starts_with("https://discord.com/api/webhooks/") 
                        && !webhook_url.starts_with("https://discordapp.com/api/webhooks/") {
                        show_webhook_message("Invalid Discord webhook URL", true);
                    } else {
                        // Clone all data we need BEFORE spawning thread
                        let report_urls = STATE.report_urls.lock().unwrap().clone();
                        let report_name = REPORT_NAME_BUFFER.with(|buffer| {
                            let name = buffer.borrow().clone();
                            let current_date = chrono::Local::now().format("%d.%m.%y").to_string();
                            name.replace("(*DATE)", &current_date)
                        });
                        
                        // Set sending state
                        *STATE.webhook_sending.lock().unwrap() = true;
                        
                        // Spawn thread with all cloned data
                        std::thread::spawn(move || {
                            log::info!("Discord webhook thread started");
                            
                            // Build a single message with all reports
                            let mut message_parts = Vec::new();
                            
                            for report_url in report_urls.iter() {
                                let is_legacy = report_url.contains("Legacy") || report_url.to_lowercase().contains("legacy");
                                
                                // Format each report link
                                let link = if is_legacy {
                                    format!("[Legacy Report]({})", report_url)
                                } else {
                                    format!("[{}]({})", report_name, report_url)
                                };
                                
                                message_parts.push(link);
                            }
                            
                            // Join all parts with newline and dash separator
                            let full_message = message_parts.join("   \n-\n");
                            
                            log::info!("Sending Discord message");
                            
                            // Send single message with all reports
                            match send_to_discord(&webhook_url, &full_message) {
                                Ok(_) => {
                                    log::info!("All reports sent to Discord successfully");
                                    
                                    // Update webhook usage
                                    let mut webhook_settings = WebhookSettings::get();
                                    webhook_settings.update_webhook_usage(&webhook_url);
                                    
                                    // Save remembered webhook if needed
                                    if remember {
                                        webhook_settings.remember_last_webhook = true;
                                        webhook_settings.last_webhook_url = webhook_url.clone();
                                    } else {
                                        webhook_settings.remember_last_webhook = false;
                                        webhook_settings.last_webhook_url.clear();
                                    }
                                    
                                    if let Err(e) = webhook_settings.store(crate::webhooks_path()) {
                                        log::error!("Failed to save webhook settings: {}", e);
                                    }
                                    
                                    drop(webhook_settings);
                                    
                                    // Update status on main thread
                                    show_webhook_message("All reports sent successfully!", false);
                                    
                                    // Close modal after a delay
                                    std::thread::sleep(std::time::Duration::from_secs(1));
                                    *STATE.show_webhook_modal.lock().unwrap() = false;
                                }
                                Err(e) => {
                                    log::error!("Failed to send reports to Discord: {}", e);
                                    show_webhook_message(&format!("Failed to send: {}", e), true);
                                }
                            }
                            
                            *STATE.webhook_sending.lock().unwrap() = false;
                            log::info!("Discord webhook thread finished");
                        });
                    }
                }

                ui.same_line();

                if ui.button("Cancel") {
                    *STATE.show_webhook_modal.lock().unwrap() = false;
                }
            }
        });
}

fn show_webhook_message(message: &str, is_error: bool) {
    // Create the values we need first
    let message_string = message.to_string();
    let until_time = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
    
    // Do all locks in sequence, dropping each immediately to prevent deadlock
    {
        let mut msg_lock = STATE.webhook_status_message.lock().unwrap();
        *msg_lock = message_string;
    }
    
    {
        let mut err_lock = STATE.webhook_status_is_error.lock().unwrap();
        *err_lock = is_error;
    }
    
    {
        let mut until_lock = STATE.webhook_status_until.lock().unwrap();
        *until_lock = until_time;
    }
}

/// Marks successfully uploaded logs in the uploaded logs tracker
fn mark_uploaded_logs() {
    let logs = STATE.logs.lock().unwrap();
    let mut uploaded = UploadedLogs::get();
    
    let mut newly_added = 0;
    for log in logs.iter() {
        if log.selected && log.uploaded {
            if !uploaded.is_uploaded(&log.filename) {
                uploaded.add_log(log.filename.clone());
                newly_added += 1;
            }
        }
    }
    
    if newly_added > 0 {
        log::info!("Marked {} new logs as uploaded", newly_added);
        
        // Save to disk
        if let Err(e) = uploaded.store(crate::uploaded_logs_path()) {
            log::error!("Failed to save uploaded logs: {}", e);
        }
    }
    
    drop(uploaded);
    drop(logs);
}
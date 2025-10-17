use nexus::imgui::Ui;

use crate::cleanup::cleanup_old_logs;
use crate::settings::Settings;
use crate::state::STATE;

/// Renders the cleanup settings tab
pub fn render_cleanup_tab(ui: &Ui) {
    thread_local! {
        static CLEANUP_DAYS: std::cell::Cell<i32> = const { std::cell::Cell::new(30) };
    }

    let cleanup_result = STATE.cleanup_result.lock().unwrap().take();
    if let Some(result) = cleanup_result {
        match result {
            Ok((_files, bytes)) => {
                let _mb = bytes as f64 / 1024.0 / 1024.0;
                *STATE.cleanup_message_until.lock().unwrap() =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            }
            Err(_) => {
                *STATE.cleanup_message_until.lock().unwrap() =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            }
        }
    }

    ui.text_colored([1.0, 0.8, 0.2, 1.0], "Log Cleanup");
    ui.spacing();
    ui.text_wrapped("Move old ArcDps log files to Recycle Bin to free up disk space.");
    ui.spacing();
    ui.separator();
    ui.spacing();

    // Auto-cleanup section
    let settings = Settings::get();
    let mut auto_enabled = settings.auto_cleanup_enabled;
    let mut auto_days = settings.auto_cleanup_days as i32;
    drop(settings);

    ui.text_colored([1.0, 1.0, 0.0, 1.0], "Automatic Cleanup");
    ui.spacing();

    if ui.checkbox("Enable automatic cleanup on plugin load", &mut auto_enabled) {
        if auto_enabled {
            // Show warning when enabling
            ui.open_popup("auto_cleanup_warning");
        } else {
            // Save immediately when disabling
            let mut settings = Settings::get();
            settings.auto_cleanup_enabled = false;
            if let Err(e) = settings.store(crate::config_path()) {
                log::error!("Failed to save settings: {}", e);
            }
        }
    }

    // Warning popup when enabling auto-cleanup
    ui.popup_modal("auto_cleanup_warning")
        .always_auto_resize(true)
        .build(ui, || {
            ui.text_colored([1.0, 0.0, 0.0, 1.0], "!WARNING!");
            ui.spacing();
            ui.text_wrapped("Automatic cleanup will run ONCE when the plugin loads");
            ui.text_wrapped("(each time you start Guild Wars 2).");
            ui.spacing();
            ui.text_wrapped("Old logs will be moved to the Recycle Bin automatically");
            ui.text_wrapped("without confirmation.");
            ui.spacing();
            ui.separator();
            ui.spacing();

            if ui.button("Enable Automatic Cleanup") {
                ui.close_current_popup();
                let mut settings = Settings::get();
                settings.auto_cleanup_enabled = true;
                if let Err(e) = settings.store(crate::config_path()) {
                    log::error!("Failed to save settings: {}", e);
                }
            }

            ui.same_line();

            if ui.button("Cancel") {
                ui.close_current_popup();
            }
        });

    if auto_enabled {
        ui.text_colored(
            [0.7, 0.7, 0.7, 1.0],
            "Auto-cleanup will run once per session",
        );
        ui.spacing();

        ui.text("Delete logs older than:");
        ui.set_next_item_width(100.0);
        if ui.input_int("##auto_cleanup_days", &mut auto_days).build() {
            auto_days = auto_days.max(1).min(9999);
            let mut settings = Settings::get();
            settings.auto_cleanup_days = auto_days as u32;
            if let Err(e) = settings.store(crate::config_path()) {
                log::error!("Failed to save settings: {}", e);
            }
        }
        ui.same_line();
        ui.text("days");
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // Manual cleanup section
    ui.text_colored([1.0, 1.0, 1.0, 1.0], "Manual Cleanup");
    ui.spacing();
    ui.text("Delete ArcDps logs older than:");
    ui.spacing();

    let mut days = CLEANUP_DAYS.get();
    ui.set_next_item_width(100.0);
    if ui.input_int("##cleanup_days", &mut days).build() {
        days = days.max(1).min(9999);
        CLEANUP_DAYS.set(days);
    }
    ui.same_line();
    ui.text("days");

    ui.spacing();
    ui.separator();
    ui.spacing();

    let settings = Settings::get();
    let log_dir = settings.log_directory.clone();
    drop(settings);

    if log_dir.is_empty() {
        ui.text_colored([1.0, 0.0, 0.0, 1.0], "No log directory configured!");
        ui.spacing();
        ui.text_wrapped("Please set a log directory in the General tab first.");
    } else {
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "Target directory:");
        ui.text_wrapped(&log_dir);
        ui.spacing();

        ui.spacing();
        ui.text_colored(
            [1.0, 0.8, 0.0, 1.0],
            "!!WARNING: Files will be moved to Recycle Bin",
        );
        ui.text_colored(
            [0.7, 0.7, 0.7, 1.0],
            "You can restore them from the Recycle Bin if needed",
        );
        ui.spacing();

        let is_cleaning = *STATE.cleanup_in_progress.lock().unwrap();

        if is_cleaning {
            let _style =
                ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
            let _style2 =
                ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
            let _style3 =
                ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
            ui.button("Cleaning...");
        } else {
            let _style =
                ui.push_style_color(nexus::imgui::StyleColor::Button, [0.8, 0.2, 0.2, 1.0]);
            let _style2 =
                ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [1.0, 0.3, 0.3, 1.0]);
            let _style3 =
                ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.6, 0.1, 0.1, 1.0]);

            if ui.button("Delete Old Logs") {
                ui.open_popup("confirm_cleanup");
            }
        }

        ui.popup_modal("confirm_cleanup")
            .always_auto_resize(true)
            .build(ui, || {
                ui.text_colored([1.0, 0.0, 0.0, 1.0], "FINAL WARNING!");
                ui.spacing();
                ui.text_wrapped(&format!(
                    "You are about to move all .zevtc files older than {} days to the Recycle Bin from:",
                    days
                ));
                ui.spacing();
                ui.text_colored([1.0, 1.0, 0.0, 1.0], &log_dir);
                ui.spacing();
                ui.text_colored(
                    [1.0, 1.0, 0.0, 1.0],
                    "Files can be restored from the Recycle Bin if needed.",
                );
                ui.spacing();
                ui.separator();
                ui.spacing();

                let _style =
                    ui.push_style_color(nexus::imgui::StyleColor::Button, [0.8, 0.2, 0.2, 1.0]);
                let _style2 = ui.push_style_color(
                    nexus::imgui::StyleColor::ButtonHovered,
                    [1.0, 0.3, 0.3, 1.0],
                );
                let _style3 = ui.push_style_color(
                    nexus::imgui::StyleColor::ButtonActive,
                    [0.6, 0.1, 0.1, 1.0],
                );

                if ui.button("Yes, Move to Recycle Bin") {
                    ui.close_current_popup();
                    *STATE.cleanup_in_progress.lock().unwrap() = true;

                    let days_to_delete = days as u32;
                    let dir_to_clean = log_dir.clone();

                    std::thread::spawn(move || {
                        let result = cleanup_old_logs(&dir_to_clean, days_to_delete);
                        *STATE.cleanup_result.lock().unwrap() = Some(result);
                        *STATE.cleanup_in_progress.lock().unwrap() = false;
                    });
                }

                ui.same_line();

                if ui.button("Cancel") {
                    ui.close_current_popup();
                }
            });

        let last_result = STATE.cleanup_result.lock().unwrap();
        let message_until = *STATE.cleanup_message_until.lock().unwrap();

        if let Some(until) = message_until {
            if std::time::Instant::now() < until {
                ui.spacing();
                ui.separator();
                ui.spacing();

                if let Some(ref result) = *last_result {
                    match result {
                        Ok((files, bytes)) => {
                            let mb = *bytes as f64 / 1024.0 / 1024.0;
                            ui.text_colored(
                                [0.0, 1.0, 0.0, 1.0],
                                &format!(
                                    "Cleanup complete: {} files deleted, {:.2} MB freed",
                                    files, mb
                                ),
                            );
                        }
                        Err(e) => {
                            ui.text_colored([1.0, 0.0, 0.0, 1.0], &format!("✗ {}", e));
                        }
                    }
                }
            } else {
                drop(last_result);
                *STATE.cleanup_message_until.lock().unwrap() = None;
            }
        } else {
            drop(last_result);
        }
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // Upload history management
    ui.text_colored([1.0, 1.0, 0.0, 1.0], "Upload History");
    ui.spacing();
    ui.text_wrapped("Clear the list of previously uploaded logs.\nThis won't delete any files, just resets the green highlighting in the log selection screen.\n");
    ui.spacing();

    let uploaded = crate::uploaded_logs::UploadedLogs::get();
    let count = uploaded.filenames.len();
    drop(uploaded);

    ui.text(format!("Currently tracking {} uploaded logs", count));
    ui.spacing();

    if ui.button("Clear Upload History") {
        ui.open_popup("confirm_clear_history");
    }

    ui.popup_modal("confirm_clear_history")
        .always_auto_resize(true)
        .build(ui, || {
            ui.text("Clear upload history?");
            ui.spacing();
            ui.text_wrapped("This will remove the green highlighting from all previously uploaded logs.");
            ui.spacing();
            ui.text_wrapped("No files will be deleted - this only resets the tracking.");
            ui.spacing();

            if ui.button("Yes, Clear History") {
                ui.close_current_popup();
                let mut uploaded = crate::uploaded_logs::UploadedLogs::get();
                uploaded.clear();
                if let Err(e) = uploaded.store(crate::uploaded_logs_path()) {
                    log::error!("Failed to save cleared upload history: {}", e);
                } else {
                    log::info!("Upload history cleared successfully");
                }
            }

            ui.same_line();

            if ui.button("Cancel") {
                ui.close_current_popup();
            }
        });
}
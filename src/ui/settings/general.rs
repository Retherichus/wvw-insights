use nexus::imgui::Ui;

use crate::arcdps::sync_with_arcdps;
use crate::settings::Settings;
use crate::state::STATE;

// Move thread_local to module level so both functions can access them
thread_local! {
    static LOG_DIR_BUFFER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static API_ENDPOINT_BUFFER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static SHOW_FORMATTED: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static INITIALIZED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Renders the general settings tab
pub fn render_general_tab(ui: &Ui, _config_path: &std::path::Path) {
    if !INITIALIZED.get() {
        let settings = Settings::get();
        LOG_DIR_BUFFER.set(settings.log_directory.clone());
        API_ENDPOINT_BUFFER.set(settings.api_endpoint.clone());
        SHOW_FORMATTED.set(settings.show_formatted_timestamps);
        INITIALIZED.set(true);
    }

    // Check if sync operation completed
    let sync_result = STATE.sync_arcdps_result.lock().unwrap().take();
    if let Some(result) = sync_result {
        match result {
            Ok(path) => {
                LOG_DIR_BUFFER.set(path);
                *STATE.sync_arcdps_message.lock().unwrap() = "Synced successfully!".to_string();
                *STATE.sync_arcdps_message_is_error.lock().unwrap() = false;
                *STATE.sync_arcdps_message_until.lock().unwrap() =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
            }
            Err(e) => {
                *STATE.sync_arcdps_message.lock().unwrap() = format!("⚠ {}", e);
                *STATE.sync_arcdps_message_is_error.lock().unwrap() = true;
                *STATE.sync_arcdps_message_until.lock().unwrap() =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            }
        }
    }

    ui.text("Log Directory:");

    // Log directory input
    LOG_DIR_BUFFER.with_borrow_mut(|dir| {
        ui.input_text("##logdir", dir)
            .hint("e.g., D:\\LOGS\\arcdps.cbtlogs\\1")
            .build();
    });

    // Sync with ArcDPS button (same line)
    ui.same_line();

    let is_syncing = *STATE.sync_arcdps_pending.lock().unwrap();
    if is_syncing {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Syncing...");
    } else {
        if ui.button("Sync with ArcDPS") {
            *STATE.sync_arcdps_pending.lock().unwrap() = true;
            std::thread::spawn(|| {
                let result = sync_with_arcdps();
                *STATE.sync_arcdps_result.lock().unwrap() = Some(result);
                *STATE.sync_arcdps_pending.lock().unwrap() = false;
            });
        }
    }

    // Show temporary message next to button
    let message_until = *STATE.sync_arcdps_message_until.lock().unwrap();
    if let Some(until) = message_until {
        if std::time::Instant::now() < until {
            ui.same_line();
            let message = STATE.sync_arcdps_message.lock().unwrap().clone();
            let is_error = *STATE.sync_arcdps_message_is_error.lock().unwrap();

            let color = if is_error {
                [1.0, 0.5, 0.0, 1.0] // Orange for errors
            } else {
                [0.0, 1.0, 0.0, 1.0] // Green for success
            };

            ui.text_colored(color, &message);
        } else {
            // Message expired, clear it
            *STATE.sync_arcdps_message_until.lock().unwrap() = None;
        }
    }

    ui.text_colored(
        [0.7, 0.7, 0.7, 1.0],
        "The folder containing your .zevtc log files",
    );
    ui.text_colored(
        [0.7, 0.7, 0.7, 1.0],
        "Subdirectories will be scanned recursively",
    );

    ui.spacing();
    ui.separator();
    ui.spacing();

    ui.text("Display Options:");
    let mut show_formatted = SHOW_FORMATTED.get();
    if ui.checkbox("Show formatted timestamps", &mut show_formatted) {
        SHOW_FORMATTED.set(show_formatted);
    }
    ui.text_colored(
        [0.7, 0.7, 0.7, 1.0],
        "Display readable dates instead of raw filenames",
    );

    ui.spacing();
    ui.separator();
    ui.spacing();

    ui.text("API Endpoint:");
    API_ENDPOINT_BUFFER.with_borrow_mut(|endpoint| {
        ui.input_text("##apiendpoint", endpoint).build();
    });

    if ui.button("Reset to Default") {
        API_ENDPOINT_BUFFER.set("https://parser.rethl.net/api.php".to_string());
    }

    ui.text_colored(
        [0.7, 0.7, 0.7, 1.0],
        "Leave as default unless instructed otherwise",
    );
}

/// Saves the general settings to config
pub fn save_general_settings(config_path: &std::path::Path) {
    LOG_DIR_BUFFER.with_borrow(|dir| {
        API_ENDPOINT_BUFFER.with_borrow(|endpoint| {
            let mut settings = Settings::get();
            settings.log_directory = dir.clone();
            settings.api_endpoint = endpoint.clone();
            settings.show_formatted_timestamps = SHOW_FORMATTED.get();

            if let Err(e) = settings.store(config_path) {
                log::error!("Failed to save settings: {}", e);
            } else {
                log::info!("Settings saved - log_directory: '{}', api_endpoint: '{}'", dir, endpoint);
            }
        });
    });
}

/// Resets the initialization flag so settings reload next time
pub fn reset_initialization() {
    INITIALIZED.set(false);
}
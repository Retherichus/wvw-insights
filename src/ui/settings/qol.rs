use nexus::imgui::Ui;

use crate::settings::Settings;

thread_local! {
    static MOUSE_LOCK_ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static INITIALIZED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Renders the QoL settings tab
pub fn render_qol_tab(ui: &Ui, _config_path: &std::path::Path) {
    if !INITIALIZED.get() {
        let settings = Settings::get();
        MOUSE_LOCK_ENABLED.set(settings.mouse_lock_enabled);
        INITIALIZED.set(true);
    }

    ui.text_colored([1.0, 1.0, 0.0, 1.0], "Quality of Life Features");
    ui.spacing();
    ui.text_colored([0.7, 0.7, 0.7, 1.0], "Optional enhancements for your GW2 experience");
    ui.spacing();
    ui.separator();
    ui.spacing();

    // Mouse lock option
    let mut mouse_lock = MOUSE_LOCK_ENABLED.get();
    if ui.checkbox("Lock mouse to game window", &mut mouse_lock) {
        MOUSE_LOCK_ENABLED.set(mouse_lock);
        
        // Apply immediately
        if mouse_lock {
            crate::qol::enable_mouse_lock();
        } else {
            crate::qol::disable_mouse_lock();
        }
    }
    
    ui.text_colored(
        [0.7, 0.7, 0.7, 1.0],
        "Prevents mouse from leaving the game window while playing",
    );
    ui.text_colored(
        [0.7, 0.7, 0.7, 1.0],
        "Automatically disabled when you tab out or lose focus",
    );
}

/// Saves QoL settings
pub fn save_qol_settings(config_path: &std::path::Path) {
    let mut settings = Settings::get();
    settings.mouse_lock_enabled = MOUSE_LOCK_ENABLED.get();
    
    if let Err(e) = settings.store(config_path) {
        log::error!("Failed to save QoL settings: {}", e);
    }
}

/// Resets initialization
pub fn reset_initialization() {
    INITIALIZED.set(false);
}
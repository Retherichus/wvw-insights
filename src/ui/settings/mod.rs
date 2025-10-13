pub mod cleanup;
pub mod general;
pub mod history;
pub mod qol;
pub mod tokens;

use nexus::imgui::Ui;

use crate::state::STATE;

/// Renders the settings screen with tabs
pub fn render_settings(ui: &Ui, config_path: &std::path::Path) {
    thread_local! {
        static ACTIVE_TAB: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    ui.text("Settings");
    ui.separator();
    ui.spacing();

    // Tab buttons
    let mut active_tab = ACTIVE_TAB.get();

    if ui.button("General") {
        active_tab = 0;
        ACTIVE_TAB.set(0);
    }
    ui.same_line();
    if ui.button("Token Manager") {
        active_tab = 1;
        ACTIVE_TAB.set(1);
    }
    ui.same_line();
    if ui.button("Report History") {
        active_tab = 2;
        ACTIVE_TAB.set(2);
    }
    ui.same_line();
    if ui.button("Cleanup") {
        active_tab = 3;
        ACTIVE_TAB.set(3);
    }
    ui.same_line();
    if ui.button("QoL") {
        active_tab = 4;
        ACTIVE_TAB.set(4);
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // Tab content
    match active_tab {
        0 => general::render_general_tab(ui, config_path),
        1 => tokens::render_tokens_tab(ui, config_path),
        2 => history::render_history_tab(ui, config_path),
        3 => cleanup::render_cleanup_tab(ui),
        4 => qol::render_qol_tab(ui, config_path),  // ADD THIS
        _ => {}
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    if ui.button("Save & Return") {
        general::save_general_settings(config_path);
        qol::save_qol_settings(config_path);

        *STATE.show_settings.lock().unwrap() = false;
        *STATE.show_token_input.lock().unwrap() = true;
        general::reset_initialization();
        qol::reset_initialization();
    }
}
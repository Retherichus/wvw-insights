pub mod cleanup;
pub mod general;
pub mod history;
pub mod qol;
pub mod tokens;
pub mod webhooks;

use nexus::imgui::Ui;

use crate::state::STATE;

thread_local! {
    static ACTIVE_TAB: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Sets the active settings tab (used when navigating from other screens)
pub fn set_active_settings_tab(tab: usize) {
    ACTIVE_TAB.set(tab);
}

/// Renders the settings screen with tabs
/// Renders the settings screen with tabs
pub fn render_settings(ui: &Ui, config_path: &std::path::Path) {
    ui.text("Settings");
    ui.separator();
    ui.spacing();

    // Tab buttons with highlighting
    let mut active_tab = ACTIVE_TAB.get();

    // General button
    if active_tab == 0 {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.4, 0.4, 0.5, 1.0]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.45, 0.45, 0.55, 1.0]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.5, 0.5, 0.6, 1.0]);
        ui.button("General");
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.25, 0.25, 0.3, 0.6]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.35, 0.8]);
        if ui.button("General") {
            active_tab = 0;
            ACTIVE_TAB.set(0);
        }
    }
    
    ui.same_line();
    
    // Token Manager button
    if active_tab == 1 {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.4, 0.4, 0.5, 1.0]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.45, 0.45, 0.55, 1.0]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.5, 0.5, 0.6, 1.0]);
        ui.button("Token Manager");
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.25, 0.25, 0.3, 0.6]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.35, 0.8]);
        if ui.button("Token Manager") {
            active_tab = 1;
            ACTIVE_TAB.set(1);
        }
    }
    
    ui.same_line();
    
    // Report History button
    if active_tab == 2 {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.4, 0.4, 0.5, 1.0]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.45, 0.45, 0.55, 1.0]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.5, 0.5, 0.6, 1.0]);
        ui.button("Report History");
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.25, 0.25, 0.3, 0.6]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.35, 0.8]);
        if ui.button("Report History") {
            active_tab = 2;
            ACTIVE_TAB.set(2);
        }
    }
    
    ui.same_line();
    
    // Webhooks button
    if active_tab == 3 {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.4, 0.4, 0.5, 1.0]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.45, 0.45, 0.55, 1.0]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.5, 0.5, 0.6, 1.0]);
        ui.button("Webhooks");
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.25, 0.25, 0.3, 0.6]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.35, 0.8]);
        if ui.button("Webhooks") {
            active_tab = 3;
            ACTIVE_TAB.set(3);
        }
    }
    
    ui.same_line();
    
    // Cleanup button
    if active_tab == 4 {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.4, 0.4, 0.5, 1.0]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.45, 0.45, 0.55, 1.0]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.5, 0.5, 0.6, 1.0]);
        ui.button("Cleanup");
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.25, 0.25, 0.3, 0.6]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.35, 0.8]);
        if ui.button("Cleanup") {
            active_tab = 4;
            ACTIVE_TAB.set(4);
        }
    }
    
    ui.same_line();
    
    // QoL button
    if active_tab == 5 {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.4, 0.4, 0.5, 1.0]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.45, 0.45, 0.55, 1.0]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.5, 0.5, 0.6, 1.0]);
        ui.button("QoL");
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.25, 0.25, 0.3, 0.6]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.35, 0.8]);
        if ui.button("QoL") {
            active_tab = 5;
            ACTIVE_TAB.set(5);
        }
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // Tab content
    match active_tab {
        0 => general::render_general_tab(ui, config_path),
        1 => tokens::render_tokens_tab(ui, config_path),
        2 => history::render_history_tab(ui, config_path),
        3 => webhooks::render_webhooks_tab(ui, config_path),
        4 => cleanup::render_cleanup_tab(ui),
        5 => qol::render_qol_tab(ui, config_path),
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
use nexus::imgui::{ChildWindow, Ui};

use crate::formatting::format_report_timestamp;
use crate::report_history::ReportHistory;
use crate::settings::Settings;

/// Renders the report history tab
pub fn render_history_tab(ui: &Ui, _config_path: &std::path::Path) {
    thread_local! {
        static REPORT_TO_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    }

    ui.text("Your Report History:");
    ui.spacing();

    let settings = Settings::get();
    let current_token = settings.history_token.clone();
    drop(settings);

    let history = ReportHistory::get();
    let mut reports = history.reports.clone();
    drop(history);

    // Sort by timestamp (newest first)
    reports.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    if reports.is_empty() {
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "No reports yet");
        ui.spacing();
        ui.text_colored(
            [0.7, 0.7, 0.7, 1.0],
            "Complete a parse to see it here!",
        );
    } else {
        ui.text_colored(
            [0.7, 0.7, 0.7, 1.0],
            &format!("Total sessions: {}", reports.len()),
        );
        ui.spacing();

        if ui.button("Clear All History") {
            ui.open_popup("clear_history_confirmation");
        }

        ui.popup_modal("clear_history_confirmation")
            .always_auto_resize(true)
            .build(ui, || {
                ui.text("Are you sure you want to clear all report history?");
                ui.spacing();
                ui.text_colored([1.0, 1.0, 0.0, 1.0], "This cannot be undone!");
                ui.spacing();

                if ui.button("Yes, Clear All") {
                    ui.close_current_popup();
                    let mut history = ReportHistory::get();
                    history.clear();
                    if let Err(e) = history.store(crate::report_history_path()) {
                        log::error!("Failed to save history: {}", e);
                    }
                    log::info!("Cleared all report history");
                }

                ui.same_line();

                if ui.button("Cancel") {
                    ui.close_current_popup();
                }
            });

        ui.spacing();
        ui.separator();
        ui.spacing();

        ChildWindow::new("ReportHistoryList")
            .size([0.0, 350.0])
            .build(ui, || {
                for (index, entry) in reports.iter().enumerate() {
                    let timestamp_str = format_report_timestamp(entry.timestamp);

                    ui.text_colored([0.8, 0.8, 1.0, 1.0], &timestamp_str);
                    ui.text_colored(
                        [0.6, 0.6, 0.6, 1.0],
                        &format!("Session: {}", entry.session_id),
                    );
                    ui.spacing();

                    // Main Report section
                    ui.text_colored([0.9, 0.9, 1.0, 1.0], "Main Report:");
                    ui.same_line();

                    if ui.small_button(&format!("Copy URL##copy_main_{}", index)) {
                        ui.set_clipboard_text(&entry.main_report_url);
                        log::info!("Copied main report URL to clipboard");
                    }

                    ui.same_line();

                    if ui.small_button(&format!("Open##open_main_{}", index)) {
                        if let Err(e) = open::that_detached(&entry.main_report_url) {
                            log::error!("Failed to open browser: {}", e);
                        }
                    }

                    // Legacy Report section (if it exists)
                    if let Some(ref legacy_url) = entry.legacy_report_url {
                        ui.text_colored([0.8, 0.8, 0.6, 1.0], "Legacy Report:");
                        ui.same_line();

                        if ui.small_button(&format!("Copy URL##copy_legacy_{}", index)) {
                            ui.set_clipboard_text(legacy_url);
                            log::info!("Copied legacy report URL to clipboard");
                        }

                        ui.same_line();

                        if ui.small_button(&format!("Open##open_legacy_{}", index)) {
                            if let Err(e) = open::that_detached(legacy_url) {
                                log::error!("Failed to open browser: {}", e);
                            }
                        }
                    }

                    // Delete button for the entire session
                    if ui.small_button(&format!("Delete Session##del_{}", index)) {
                        REPORT_TO_DELETE.set(Some(index));
                    }

                    ui.spacing();
                    ui.separator();
                    ui.spacing();
                }
            });
    }

    // Handle deletion
    if let Some(index_to_delete) = REPORT_TO_DELETE.get() {
        let mut history = ReportHistory::get();
        // Sort the same way to match indices
        history.reports.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        history.remove_report(index_to_delete);
        if let Err(e) = history.store(crate::report_history_path()) {
            log::error!("Failed to save history after deletion: {}", e);
        } else {
            log::info!("Deleted report session from history");
        }
        REPORT_TO_DELETE.set(None);
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // View all reports on website button
    ui.text_colored(
        [0.7, 0.7, 0.7, 1.0],
        "View all reports parsed with your current token:",
    );
    ui.spacing();

    if !current_token.is_empty() {
        if ui.button("View All Reports on Website") {
            let url = format!("https://parser.rethl.net/?hisToken={}", current_token);
            if let Err(e) = open::that_detached(&url) {
                log::error!("Failed to open browser: {}", e);
            } else {
                log::info!("Opening all reports on website with token");
            }
        }

        ui.same_line();

        if ui.small_button("Copy Link") {
            let url = format!("https://parser.rethl.net/?hisToken={}", current_token);
            ui.set_clipboard_text(&url);
            log::info!("Copied website URL to clipboard");
        }
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("View All Reports on Website");
        drop(_style3);
        drop(_style2);
        drop(_style);

        if ui.is_item_hovered() {
            ui.tooltip_text("Enter a history token first");
        }
    }
}
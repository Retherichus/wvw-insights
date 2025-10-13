use nexus::imgui::{ChildWindow, Ui};

use crate::settings::{SavedToken, Settings};

/// Renders the token manager tab
pub fn render_tokens_tab(ui: &Ui, config_path: &std::path::Path) {
    thread_local! {
        static NEW_TOKEN_NAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static NEW_TOKEN_VALUE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static TOKEN_TO_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    }

    ui.text("Saved Tokens:");
    ui.spacing();

    let settings = Settings::get();
    let saved_tokens = settings.saved_tokens.clone();
    drop(settings);

    if saved_tokens.is_empty() {
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "No saved tokens yet");
    } else {
        ChildWindow::new("SavedTokensList")
            .size([0.0, 200.0])
            .build(ui, || {
                for (index, saved_token) in saved_tokens.iter().enumerate() {
                    ui.text(&saved_token.name);
                    ui.same_line();

                    let masked = if saved_token.token.len() > 8 {
                        format!(
                            "{}...{}",
                            &saved_token.token[..4],
                            &saved_token.token[saved_token.token.len() - 4..]
                        )
                    } else {
                        "****".to_string()
                    };
                    ui.text_colored([0.5, 0.5, 0.5, 1.0], &masked);

                    ui.same_line();

                    if ui.small_button(&format!("Use##use_{}", index)) {
                        let mut settings = Settings::get();
                        settings.history_token = saved_token.token.clone();
                        if let Err(e) = settings.store(config_path) {
                            log::error!("Failed to save settings: {}", e);
                        }
                        log::info!("Switched to token: {}", saved_token.name);
                    }

                    ui.same_line();

                    if ui.small_button(&format!("Delete##del_{}", index)) {
                        TOKEN_TO_DELETE.set(Some(index));
                    }

                    ui.spacing();
                }
            });
    }

    if let Some(index_to_delete) = TOKEN_TO_DELETE.get() {
        let mut settings = Settings::get();
        if index_to_delete < settings.saved_tokens.len() {
            let deleted_name = settings.saved_tokens[index_to_delete].name.clone();
            settings.saved_tokens.remove(index_to_delete);
            if let Err(e) = settings.store(config_path) {
                log::error!("Failed to save settings after deletion: {}", e);
            } else {
                log::info!("Deleted token: {}", deleted_name);
            }
        }
        TOKEN_TO_DELETE.set(None);
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    ui.text("Save New Token:");
    ui.spacing();

    ui.text_colored([0.9, 0.9, 0.9, 1.0], "Token Name:");
    NEW_TOKEN_NAME.with_borrow_mut(|name| {
        ui.input_text("##newTokenName", name).build();
    });
    ui.text_colored(
        [0.6, 0.6, 0.6, 1.0],
        "(e.g., Main Account, Alt Account)",
    );

    ui.spacing();

    ui.text_colored([0.9, 0.9, 0.9, 1.0], "Token Value:");
    NEW_TOKEN_VALUE.with_borrow_mut(|token| {
        ui.input_text("##newTokenValue", token).build();
    });
    ui.text_colored([0.6, 0.6, 0.6, 1.0], "(Paste your history token here)");

    ui.spacing();

    let can_save = NEW_TOKEN_NAME.with_borrow(|name| !name.trim().is_empty())
        && NEW_TOKEN_VALUE.with_borrow(|token| !token.trim().is_empty());

    if can_save {
        if ui.button("Save Token") {
            NEW_TOKEN_NAME.with_borrow(|name| {
                NEW_TOKEN_VALUE.with_borrow(|token| {
                    let mut settings = Settings::get();
                    settings.saved_tokens.push(SavedToken {
                        name: name.trim().to_string(),
                        token: token.trim().to_string(),
                    });
                    if let Err(e) = settings.store(config_path) {
                        log::error!("Failed to save token: {}", e);
                    } else {
                        log::info!("Saved new token: {}", name.trim());
                    }
                });
            });

            NEW_TOKEN_NAME.set(String::new());
            NEW_TOKEN_VALUE.set(String::new());
        }
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Save Token");
    }
}
use nexus::imgui::{ChildWindow, Ui};

use crate::settings::{SavedToken, Settings};
use crate::state::STATE;
use crate::tokens::validate_token;

/// Renders the token manager tab
pub fn render_tokens_tab(ui: &Ui, config_path: &std::path::Path) {
    thread_local! {
        static NEW_TOKEN_NAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static NEW_TOKEN_VALUE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static TOKEN_TO_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    }

    // Show applied message at the top if active
    let applied_message_until = *STATE.token_applied_message_until.lock().unwrap();
    if let Some(until) = applied_message_until {
        if std::time::Instant::now() < until {
            let message = STATE.token_applied_message.lock().unwrap().clone();
            ui.text_colored([0.0, 1.0, 0.0, 1.0], &message);
            ui.spacing();
        } else {
            // Message expired, clear it
            *STATE.token_applied_message_until.lock().unwrap() = None;
        }
    }

    ui.text("Saved Tokens:");
    ui.spacing();

    let settings = Settings::get();
    let saved_tokens = settings.saved_tokens.clone();
    let current_token = settings.history_token.clone();
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

                    // Check if this token is currently in use
                    let is_current = saved_token.token == current_token;

                    if is_current {
                        // Show "Active" button in green, disabled
                        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.0, 0.5, 0.0, 0.8]);
                        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.0, 0.5, 0.0, 0.8]);
                        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.0, 0.5, 0.0, 0.8]);
                        ui.small_button(&format!("Active##use_{}", index));
                    } else {
                        // Show regular "Use" button
                        if ui.small_button(&format!("Use##use_{}", index)) {
                            let mut settings = Settings::get();
                            settings.history_token = saved_token.token.clone();
                            
                            if let Err(e) = settings.store(config_path) {
                                log::error!("Failed to save settings: {}", e);
                            } else {
                                log::info!("Switched to token: {}", saved_token.name);
                                
                                // Set the token in STATE so token_input.rs picks it up
                                *STATE.generated_token.lock().unwrap() = saved_token.token.clone();
                                
                                // Show confirmation message
                                *STATE.token_applied_message.lock().unwrap() = format!("Key '{}' applied", saved_token.name);
                                *STATE.token_applied_message_until.lock().unwrap() = 
                                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                            }
                        }
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

    // Show validation message if active
    let validation_until = *STATE.save_token_validation_message_until.lock().unwrap();
    if let Some(until) = validation_until {
        if std::time::Instant::now() < until {
            let message = STATE.save_token_validation_message.lock().unwrap().clone();
            let is_error = *STATE.save_token_validation_is_error.lock().unwrap();

            let color = if is_error {
                [1.0, 0.3, 0.0, 1.0] // Red-orange for invalid
            } else {
                [0.0, 1.0, 0.0, 1.0] // Green for valid
            };

            ui.text_colored(color, &message);
        } else {
            // Message expired, clear it
            *STATE.save_token_validation_message_until.lock().unwrap() = None;
        }
    }

    ui.spacing();

    let can_save = NEW_TOKEN_NAME.with_borrow(|name| !name.trim().is_empty())
        && NEW_TOKEN_VALUE.with_borrow(|token| !token.trim().is_empty());
    let is_validating = *STATE.save_token_validating.lock().unwrap();

    if can_save && !is_validating {
        if ui.button("Save Token") {
            let token_to_validate = NEW_TOKEN_VALUE.with_borrow(|token| token.trim().to_string());
            let token_name = NEW_TOKEN_NAME.with_borrow(|name| name.trim().to_string());
            
            let settings = Settings::get();
            let api_endpoint = settings.api_endpoint.clone();
            let config_path = config_path.to_path_buf();
            drop(settings);
            
            // Start validation
            *STATE.save_token_validating.lock().unwrap() = true;
            STATE.save_token_validation_message.lock().unwrap().clear();
            *STATE.save_token_validation_message_until.lock().unwrap() = None;
            
            std::thread::spawn(move || {
                log::info!("Validating token before saving: {}", token_name);
                
                match validate_token(&api_endpoint, &token_to_validate) {
                    Ok(true) => {
                        log::info!("Token validation successful, saving token: {}", token_name);
                        
                        // Token is valid, save it
                        let mut settings = Settings::get();
                        settings.saved_tokens.push(SavedToken {
                            name: token_name.clone(),
                            token: token_to_validate,
                        });
                        
                        if let Err(e) = settings.store(&config_path) {
                            log::error!("Failed to save token: {}", e);
                            *STATE.save_token_validation_message.lock().unwrap() = format!("Failed to save: {}", e);
                            *STATE.save_token_validation_is_error.lock().unwrap() = true;
                        } else {
                            log::info!("Saved new token: {}", token_name);
                            *STATE.save_token_validation_message.lock().unwrap() = format!("Token '{}' saved successfully!", token_name);
                            *STATE.save_token_validation_is_error.lock().unwrap() = false;
                            
                            // Clear the input fields
                            NEW_TOKEN_NAME.set(String::new());
                            NEW_TOKEN_VALUE.set(String::new());
                        }
                        
                        *STATE.save_token_validation_message_until.lock().unwrap() = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                        *STATE.save_token_validating.lock().unwrap() = false;
                    }
                    Ok(false) => {
                        log::warn!("Token validation failed - invalid token");
                        *STATE.save_token_validation_message.lock().unwrap() = "Invalid token! Cannot save.".to_string();
                        *STATE.save_token_validation_is_error.lock().unwrap() = true;
                        *STATE.save_token_validation_message_until.lock().unwrap() = Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                        *STATE.save_token_validating.lock().unwrap() = false;
                    }
                    Err(e) => {
                        log::error!("Token validation error: {}", e);
                        *STATE.save_token_validation_message.lock().unwrap() = format!("Validation error: {}", e);
                        *STATE.save_token_validation_is_error.lock().unwrap() = true;
                        *STATE.save_token_validation_message_until.lock().unwrap() = Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                        *STATE.save_token_validating.lock().unwrap() = false;
                    }
                }
            });
        }
    } else if is_validating {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Validating...");
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Save Token");
    }
}
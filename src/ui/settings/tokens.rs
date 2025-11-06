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
        static DUPLICATE_NAME_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        
        // dps.report token management
        static NEW_DPS_TOKEN_NAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static NEW_DPS_TOKEN_VALUE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static DPS_TOKEN_TO_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
        static DPS_DUPLICATE_NAME_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        
        // Sub-tab state: 0 = History Tokens, 1 = dps.report Tokens
        static ACTIVE_SUB_TAB: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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

// Sub-tab navigation with subtle highlighting
    let active_sub_tab = ACTIVE_SUB_TAB.get();
    
    // History Tokens button
    if active_sub_tab == 0 {
        // Active tab - slightly brighter
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.4, 0.4, 0.5, 1.0]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.45, 0.45, 0.55, 1.0]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.5, 0.5, 0.6, 1.0]);
        ui.button("History Tokens");
    } else {
        // Inactive tab - faded
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.25, 0.25, 0.3, 0.6]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.35, 0.8]);
        if ui.button("History Tokens") {
            ACTIVE_SUB_TAB.set(0);
        }
    }
    
    ui.same_line();
    
    // dps.report Tokens button
    if active_sub_tab == 1 {
        // Active tab - slightly brighter
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.4, 0.4, 0.5, 1.0]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.45, 0.45, 0.55, 1.0]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.5, 0.5, 0.6, 1.0]);
        ui.button("dps.report Tokens");
    } else {
        // Inactive tab - faded
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.25, 0.25, 0.3, 0.6]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.35, 0.8]);
        if ui.button("dps.report Tokens") {
            ACTIVE_SUB_TAB.set(1);
        }
    }
    ui.spacing();
    ui.separator();
    ui.spacing();

    // Render content based on active sub-tab
    match ACTIVE_SUB_TAB.get() {
        0 => render_history_tokens_section(ui, config_path),
        1 => render_dps_tokens_section(ui, config_path),
        _ => {}
    }
}

/// Renders the History Tokens section
fn render_history_tokens_section(ui: &Ui, config_path: &std::path::Path) {
    thread_local! {
        static NEW_TOKEN_NAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static NEW_TOKEN_VALUE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static TOKEN_TO_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
        static DUPLICATE_NAME_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    }

    ui.text_colored([0.9, 0.7, 0.2, 1.0], "History Tokens (Parser API)");
    ui.spacing();

    ui.text("Saved History Tokens:");
    ui.spacing();

    let settings = Settings::get();
    let saved_tokens = settings.saved_tokens.clone();
    let current_token = settings.history_token.clone();
    drop(settings);

    if saved_tokens.is_empty() {
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "No saved history tokens yet");
    } else {
        ChildWindow::new("SavedTokensList")
            .size([0.0, 150.0])
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
    ui.text("Save New History Token:");
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

    // Show duplicate name error if present
    let dup_error = DUPLICATE_NAME_ERROR.with_borrow(|e| e.clone());
    if !dup_error.is_empty() {
        ui.text_colored([1.0, 0.3, 0.0, 1.0], &dup_error);
        ui.spacing();
    }

    // Show validation message if active
    let validation_until = *STATE.save_token_validation_message_until.lock().unwrap();
    if let Some(until) = validation_until {
        if std::time::Instant::now() < until {
            let message = STATE.save_token_validation_message.lock().unwrap().clone();
            let is_error = *STATE.save_token_validation_is_error.lock().unwrap();

            let color = if is_error {
                [1.0, 0.3, 0.0, 1.0]
            } else {
                [0.0, 1.0, 0.0, 1.0]
            };

            ui.text_colored(color, &message);
        } else {
            *STATE.save_token_validation_message_until.lock().unwrap() = None;
        }
    }

    ui.spacing();

    let can_save = NEW_TOKEN_NAME.with_borrow(|name| !name.trim().is_empty())
        && NEW_TOKEN_VALUE.with_borrow(|token| !token.trim().is_empty());
    let is_validating = *STATE.save_token_validating.lock().unwrap();

    if can_save && !is_validating {
        if ui.button("Save History Token") {
            let token_to_validate = NEW_TOKEN_VALUE.with_borrow(|token| token.trim().to_string());
            let token_name = NEW_TOKEN_NAME.with_borrow(|name| name.trim().to_string());
            
            let settings = Settings::get();
            let name_exists = settings.saved_tokens.iter().any(|t| t.name == token_name);
            let api_endpoint = settings.api_endpoint.clone();
            let config_path = config_path.to_path_buf();
            drop(settings);
            
            if name_exists {
                log::warn!("Token name '{}' already exists", token_name);
                DUPLICATE_NAME_ERROR.set(format!("Name '{}' already exists! Choose a different name.", token_name));
            } else {
                DUPLICATE_NAME_ERROR.set(String::new());
                
                *STATE.save_token_validating.lock().unwrap() = true;
                STATE.save_token_validation_message.lock().unwrap().clear();
                *STATE.save_token_validation_message_until.lock().unwrap() = None;
                
                std::thread::spawn(move || {
                    log::info!("Validating token before saving: {}", token_name);
                    
                    match validate_token(&api_endpoint, &token_to_validate) {
                        Ok(true) => {
                            log::info!("Token validation successful, saving token: {}", token_name);
                            
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
        }
    } else if is_validating {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Validating...");
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Save History Token");
    }
}

/// Renders the dps.report Tokens section
fn render_dps_tokens_section(ui: &Ui, config_path: &std::path::Path) {
    thread_local! {
        static NEW_DPS_TOKEN_NAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static NEW_DPS_TOKEN_VALUE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static DPS_TOKEN_TO_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
        static DPS_DUPLICATE_NAME_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    }

    ui.text_colored([0.2, 0.8, 1.0, 1.0], "dps.report Tokens");
    ui.spacing();

    ui.text("Saved dps.report Tokens:");
    ui.spacing();

    let settings = Settings::get();
    let saved_dps_tokens = settings.saved_dps_tokens.clone();
    let current_dps_token = settings.dps_report_token.clone();
    drop(settings);

    if saved_dps_tokens.is_empty() {
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "No saved dps.report tokens yet");
    } else {
        ChildWindow::new("SavedDpsTokensList")
            .size([0.0, 150.0])
            .build(ui, || {
                for (index, saved_token) in saved_dps_tokens.iter().enumerate() {
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

                    let is_current = saved_token.token == current_dps_token;

                    if is_current {
                        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.0, 0.5, 0.0, 0.8]);
                        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.0, 0.5, 0.0, 0.8]);
                        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.0, 0.5, 0.0, 0.8]);
                        ui.small_button(&format!("Active##use_dps_{}", index));
                    } else {
                        if ui.small_button(&format!("Use##use_dps_{}", index)) {
                            let mut settings = Settings::get();
                            settings.dps_report_token = saved_token.token.clone();
                            
                            if let Err(e) = settings.store(config_path) {
                                log::error!("Failed to save settings: {}", e);
                            } else {
                                log::info!("Switched to dps.report token: {}", saved_token.name);
                                
                                // Force token_input.rs to reload buffers from settings
                                crate::ui::token_input::reset_initialization();
                                
                                *STATE.token_applied_message.lock().unwrap() = format!("dps.report token '{}' applied", saved_token.name);
                                *STATE.token_applied_message_until.lock().unwrap() = 
                                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                            }
                        }
                    }

                    ui.same_line();

                    if ui.small_button(&format!("Delete##del_dps_{}", index)) {
                        DPS_TOKEN_TO_DELETE.set(Some(index));
                    }

                    ui.spacing();
                }
            });
    }
    
    if let Some(index_to_delete) = DPS_TOKEN_TO_DELETE.get() {
        let mut settings = Settings::get();
        if index_to_delete < settings.saved_dps_tokens.len() {
            let deleted_name = settings.saved_dps_tokens[index_to_delete].name.clone();
            settings.saved_dps_tokens.remove(index_to_delete);
            if let Err(e) = settings.store(config_path) {
                log::error!("Failed to save settings after deletion: {}", e);
            } else {
                log::info!("Deleted dps.report token: {}", deleted_name);
            }
        }
        DPS_TOKEN_TO_DELETE.set(None);
    }

    ui.spacing();
    ui.text("Save New dps.report Token:");
    ui.spacing();

    ui.text_colored([0.9, 0.9, 0.9, 1.0], "Token Name:");
    NEW_DPS_TOKEN_NAME.with_borrow_mut(|name| {
        ui.input_text("##newDpsTokenName", name).build();
    });
    ui.text_colored(
        [0.6, 0.6, 0.6, 1.0],
        "(e.g., Main dps.report, Alt Account)",
    );

    ui.spacing();

    ui.text_colored([0.9, 0.9, 0.9, 1.0], "Token Value:");
    NEW_DPS_TOKEN_VALUE.with_borrow_mut(|token| {
        ui.input_text("##newDpsTokenValue", token).build();
    });
    ui.text_colored([0.6, 0.6, 0.6, 1.0], "(Paste your dps.report token here)");

    ui.spacing();

    // Show duplicate name error if present
    let dps_dup_error = DPS_DUPLICATE_NAME_ERROR.with_borrow(|e| e.clone());
    if !dps_dup_error.is_empty() {
        ui.text_colored([1.0, 0.3, 0.0, 1.0], &dps_dup_error);
        ui.spacing();
    }

    let can_save_dps = NEW_DPS_TOKEN_NAME.with_borrow(|name| !name.trim().is_empty())
        && NEW_DPS_TOKEN_VALUE.with_borrow(|token| !token.trim().is_empty());

    if can_save_dps {
        if ui.button("Save dps.report Token") {
            let token_value = NEW_DPS_TOKEN_VALUE.with_borrow(|token| token.trim().to_string());
            let token_name = NEW_DPS_TOKEN_NAME.with_borrow(|name| name.trim().to_string());
            
            let mut settings = Settings::get();
            let name_exists = settings.saved_dps_tokens.iter().any(|t| t.name == token_name);
            
            if name_exists {
                log::warn!("dps.report token name '{}' already exists", token_name);
                DPS_DUPLICATE_NAME_ERROR.set(format!("Name '{}' already exists! Choose a different name.", token_name));
            } else {
                DPS_DUPLICATE_NAME_ERROR.set(String::new());
                
                settings.saved_dps_tokens.push(SavedToken {
                    name: token_name.clone(),
                    token: token_value,
                });
                
                if let Err(e) = settings.store(config_path) {
                    log::error!("Failed to save dps.report token: {}", e);
                } else {
                    log::info!("Saved new dps.report token: {}", token_name);
                    NEW_DPS_TOKEN_NAME.set(String::new());
                    NEW_DPS_TOKEN_VALUE.set(String::new());
                    
                    *STATE.token_applied_message.lock().unwrap() = format!("dps.report token '{}' saved!", token_name);
                    *STATE.token_applied_message_until.lock().unwrap() = 
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                }
            }
        }
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Save dps.report Token");
    }
}
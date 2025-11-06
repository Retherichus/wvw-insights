use nexus::imgui::Ui;

use crate::scanning::scan_for_logs;
use crate::settings::{Settings, SavedToken};
use crate::state::STATE;
use crate::tokens::{generate_token, validate_token};

// Move thread_local to module level so reset_initialization can access it
thread_local! {
    static TOKEN_BUFFER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static GUILD_NAME_BUFFER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static DPS_REPORT_TOKEN_BUFFER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) }; // ADD THIS LINE
    static INITIALIZED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SHOW_NAME_MODAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static NEW_TOKEN_NAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static PENDING_TOKEN: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

pub fn reset_initialization() {
    INITIALIZED.set(false);
}

/// Helper function to find the name of a saved token
fn find_token_name(token: &str) -> Option<String> {
    let settings = Settings::get();
    settings.saved_tokens
        .iter()
        .find(|saved| saved.token == token)
        .map(|saved| saved.name.clone())
}

/// Renders the token input screen
pub fn render_token_input(ui: &Ui, config_path: &std::path::Path) {
    // Simple initialization - just once
    if !INITIALIZED.get() {
        let settings = Settings::get();
        TOKEN_BUFFER.set(settings.history_token.clone());
        GUILD_NAME_BUFFER.set(settings.guild_name.clone());
        DPS_REPORT_TOKEN_BUFFER.set(settings.dps_report_token.clone());
        INITIALIZED.set(true);
    }

    // Check if we have a newly generated token to insert (from Generate Key or Use button)
    let generated_token = STATE.generated_token.lock().unwrap();
    if !generated_token.is_empty() {
        TOKEN_BUFFER.set(generated_token.clone());
        drop(generated_token);
        STATE.generated_token.lock().unwrap().clear();
        
        // Also save it to settings immediately
        let mut settings = Settings::get();
        settings.history_token = TOKEN_BUFFER.with_borrow(|token| token.clone());
        if let Err(e) = settings.store(config_path) {
            log::error!("Failed to save token from state: {}", e);
        }
    } else {
        drop(generated_token);
    }

    // Render the name input modal if needed
    if SHOW_NAME_MODAL.get() {
        render_name_modal(ui, config_path);
    }

    ui.text("Enter your History Token");
    ui.spacing();

    let mut token_changed = false;
    TOKEN_BUFFER.with_borrow_mut(|token| {
        if ui.input_text("##token", token).build() {
            token_changed = true;
        }
    });

    // Save token in real-time when it changes
    if token_changed {
        TOKEN_BUFFER.with_borrow(|token| {
            let mut settings = Settings::get();
            settings.history_token = token.clone();
            if let Err(e) = settings.store(config_path) {
                log::error!("Failed to save token in real-time: {}", e);
            } else {
                log::debug!("Token saved in real-time: {}", token);
            }
        });
    }

    // Display token name if it matches a saved token
    let current_token = TOKEN_BUFFER.with_borrow(|token| token.clone());
    if !current_token.is_empty() {
        if let Some(token_name) = find_token_name(&current_token) {
            ui.text_colored([0.4, 0.8, 1.0, 1.0], &format!("Using: {}", token_name));
        }
    }

    ui.spacing();
    ui.spacing();

    // Guild Name field (optional)
    ui.text("Guild Name (optional)");
    ui.spacing();

    let mut guild_name_changed = false;
    GUILD_NAME_BUFFER.with_borrow_mut(|guild_name| {
        if ui.input_text("##guildname", guild_name).build() {
            guild_name_changed = true;
        }
    });

    // Save guild name in real-time when it changes
    if guild_name_changed {
        GUILD_NAME_BUFFER.with_borrow(|guild_name| {
            let mut settings = Settings::get();
            settings.guild_name = guild_name.clone();
            if let Err(e) = settings.store(config_path) {
                log::error!("Failed to save guild name in real-time: {}", e);
            } else {
                log::debug!("Guild name saved in real-time: {}", guild_name);
            }
        });
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // dps.report Token field (optional)
    ui.text("dps.report Token (optional)");
    ui.spacing();

    let mut dps_token_changed = false;
    DPS_REPORT_TOKEN_BUFFER.with_borrow_mut(|dps_token| {
        if ui.input_text("##dpsreporttoken", dps_token).build() {
            dps_token_changed = true;
        }
    });

    // Save dps.report token in real-time when it changes
    if dps_token_changed {
        DPS_REPORT_TOKEN_BUFFER.with_borrow(|dps_token| {
            let mut settings = Settings::get();
            settings.dps_report_token = dps_token.clone();
            if let Err(e) = settings.store(config_path) {
                log::error!("Failed to save dps.report token in real-time: {}", e);
            } else {
                log::debug!("dps.report token saved in real-time: {}", dps_token);
            }
        });
    }

    // Display dps.report token name if it matches a saved token
    let current_dps_token = DPS_REPORT_TOKEN_BUFFER.with_borrow(|token| token.clone());
    if !current_dps_token.is_empty() {
        let settings = Settings::get();
        if let Some(saved_dps_token) = settings.saved_dps_tokens.iter().find(|t| t.token == current_dps_token) {
            ui.text_colored([0.4, 0.8, 1.0, 1.0], &format!("Using: {}", saved_dps_token.name));
        }
        drop(settings);
    }

    ui.spacing();

    // Warning text
    ui.text_colored([1.0, 0.5, 0.0, 1.0], "Warning: Very slow processing");
    ui.text_colored([0.7, 0.7, 0.7, 1.0], "Fight-by-fight uploads via dps.report are optional and not recommended for WvW.");
    ui.text_colored([0.7, 0.7, 0.7, 1.0], "This significantly increases processing time..");

    ui.spacing();

    // Show temporary validation message on its own line
    let message_until = *STATE.token_validation_message_until.lock().unwrap();
    if let Some(until) = message_until {
        if std::time::Instant::now() < until {
            let message = STATE.token_validation_message.lock().unwrap().clone();
            let is_error = *STATE.token_validation_is_error.lock().unwrap();

            let color = if is_error {
                [1.0, 0.3, 0.0, 1.0] // Red-orange for invalid
            } else {
                [0.0, 1.0, 0.0, 1.0] // Green for valid
            };

            ui.text_colored(color, &message);
        } else {
            // Message expired, clear it
            *STATE.token_validation_message_until.lock().unwrap() = None;
        }
    }
    
    // Show token applied message (from token manager)
    let applied_message_until = *STATE.token_applied_message_until.lock().unwrap();
    if let Some(until) = applied_message_until {
        if std::time::Instant::now() < until {
            let message = STATE.token_applied_message.lock().unwrap().clone();
            ui.text_colored([0.0, 1.0, 0.0, 1.0], &message);
        } else {
            // Message expired, clear it
            *STATE.token_applied_message_until.lock().unwrap() = None;
        }
    }

    ui.spacing();

    // Show generation status/error
    let is_generating = *STATE.token_generating.lock().unwrap();
    if is_generating {
        ui.text_colored([1.0, 1.0, 0.0, 1.0], "Generating token...");
    }
    
    let error = STATE.token_generation_error.lock().unwrap();
    if !error.is_empty() {
        ui.text_colored([1.0, 0.0, 0.0, 1.0], &*error);
    }
    drop(error);

    ui.spacing();

    let token_is_empty = TOKEN_BUFFER.with_borrow(|token| token.is_empty());
    let is_validating = *STATE.token_validating.lock().unwrap();
    
    // Continue button - only enabled if token is not empty and not validating
    if !token_is_empty && !is_validating {
        if ui.button("Continue") {
            let token_to_validate = TOKEN_BUFFER.with_borrow(|token| token.clone());
            let settings = Settings::get();
            let api_endpoint = settings.api_endpoint.clone();
            drop(settings);
            
            
            // Start validation
            *STATE.token_validating.lock().unwrap() = true;
            STATE.token_validation_message.lock().unwrap().clear();
            *STATE.token_validation_message_until.lock().unwrap() = None;
            
            std::thread::spawn(move || {
                log::info!("Validating token...");
                
                match validate_token(&api_endpoint, &token_to_validate) {
                    Ok(true) => {
                        log::info!("Token validation successful");
                        
                        // Token is already saved in real-time, just scan for logs
                        scan_for_logs();
                        
                        // Switch to log selection
                        *STATE.show_token_input.lock().unwrap() = false;
                        *STATE.show_log_selection.lock().unwrap() = true;
                        
                        *STATE.token_validating.lock().unwrap() = false;
                    }
                    Ok(false) => {
                        log::warn!("Token validation failed - invalid token");
                        *STATE.token_validation_message.lock().unwrap() = 
                            "Invalid token! Try another or generate new".to_string();
                        *STATE.token_validation_is_error.lock().unwrap() = true;
                        *STATE.token_validation_message_until.lock().unwrap() = 
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                        *STATE.token_validating.lock().unwrap() = false;
                    }
                    Err(e) => {
                        log::error!("Token validation error: {}", e);
                        *STATE.token_validation_message.lock().unwrap() = 
                            format!("Validation error: {}", e);
                        *STATE.token_validation_is_error.lock().unwrap() = true;
                        *STATE.token_validation_message_until.lock().unwrap() = 
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                        *STATE.token_validating.lock().unwrap() = false;
                    }
                }
            });
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
        ui.button("Continue");
    }
    
    ui.same_line();
    
    if ui.button("Manage Tokens") {
        *STATE.show_token_input.lock().unwrap() = false;
        *STATE.show_settings.lock().unwrap() = true;
        // Set active tab to Token Manager (tab index 1)
        crate::ui::settings::set_active_settings_tab(1);
    }
    
    ui.same_line();
    
    if ui.button("Settings") {
        *STATE.show_token_input.lock().unwrap() = false;
        *STATE.show_settings.lock().unwrap() = true;
        // Set active tab to General (tab index 0)
        crate::ui::settings::set_active_settings_tab(0);
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // Generate key button - only enabled if token field is empty and not currently generating
    let button_enabled = token_is_empty && !is_generating;
    
    if button_enabled {
        if ui.button("Generate New Token") {
            SHOW_NAME_MODAL.set(true);
            NEW_TOKEN_NAME.set(String::new());
        }
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Generate New Token");
    }
    
    if !token_is_empty && !is_generating {
        ui.same_line();
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "(Clear token field to generate new)");
    }
}

/// Renders the modal for naming a new token
fn render_name_modal(ui: &Ui, config_path: &std::path::Path) {
    thread_local! {
        static POPUP_JUST_OPENED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static DUPLICATE_NAME_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    }
    
    let should_show = SHOW_NAME_MODAL.get();
    
    // Reset the flag when modal is closed
    if !should_show {
        POPUP_JUST_OPENED.set(false);
        *STATE.token_modal_should_close.lock().unwrap() = false;
        DUPLICATE_NAME_ERROR.set(String::new());
        return;
    }
    
    // Close the popup if we got a success signal from the generation thread
    let should_close = *STATE.token_modal_should_close.lock().unwrap();
    if should_close {
        log::info!("Closing token generation modal after successful generation");
        ui.close_current_popup();
        SHOW_NAME_MODAL.set(false);
        *STATE.token_modal_should_close.lock().unwrap() = false;
        POPUP_JUST_OPENED.set(false);
        DUPLICATE_NAME_ERROR.set(String::new());
        return;
    }
    
    let is_generating = *STATE.token_generating.lock().unwrap();
    
    // Only open popup once when modal becomes visible
    if !POPUP_JUST_OPENED.get() {
        ui.open_popup("Name Your Token");
        POPUP_JUST_OPENED.set(true);
    }
    
    ui.popup_modal("Name Your Token")
        .always_auto_resize(true)
        .build(ui, || {
            ui.text("Enter a name for this token:");
            ui.text_colored([0.7, 0.7, 0.7, 1.0], "(e.g., Main Account, Alt Account, Guild Token)");
            ui.spacing();
            
            NEW_TOKEN_NAME.with_borrow_mut(|name| {
                ui.input_text("##newTokenName", name)
                    .hint("Token Name")
                    .build();
            });
            
            ui.spacing();
            
            // Show duplicate name error if present
            let dup_error = DUPLICATE_NAME_ERROR.with_borrow(|e| e.clone());
            if !dup_error.is_empty() {
                ui.text_colored([1.0, 0.3, 0.0, 1.0], &dup_error);
                ui.spacing();
            }
            
            // Show generation status
            if is_generating {
                ui.text_colored([1.0, 1.0, 0.0, 1.0], "Generating token...");
            }
            
            let error = STATE.token_generation_error.lock().unwrap();
            if !error.is_empty() {
                ui.text_colored([1.0, 0.0, 0.0, 1.0], &*error);
            }
            drop(error);
            
            ui.spacing();
            
            let name_is_empty = NEW_TOKEN_NAME.with_borrow(|name| name.trim().is_empty());
            
            // Generate button - only enabled if name is not empty and not currently generating
            if !name_is_empty && !is_generating {
                if ui.button("Generate & Save") {
                    let token_name = NEW_TOKEN_NAME.with_borrow(|name| name.trim().to_string());
                    
                    // Check if name already exists
                    let settings = Settings::get();
                    let name_exists = settings.saved_tokens.iter().any(|t| t.name == token_name);
                    drop(settings);
                    
                    if name_exists {
                        log::warn!("Token name '{}' already exists", token_name);
                        DUPLICATE_NAME_ERROR.set(format!("Name '{}' already exists! Choose a different name.", token_name));
                    } else {
                        // Clear any previous duplicate error
                        DUPLICATE_NAME_ERROR.set(String::new());
                        
                        let config_path = config_path.to_path_buf();
                        
                        log::info!("Generating token with name: {}", token_name);
                        *STATE.token_generating.lock().unwrap() = true;
                        STATE.token_generation_error.lock().unwrap().clear();
                        
                        std::thread::spawn(move || {
                            log::info!("Generating new token from server");
                            
                            match generate_token() {
                                Ok(new_token) => {
                                    log::info!("Token generated successfully: {}", new_token);
                                    
                                    // Save to settings
                                    let mut settings = Settings::get();
                                    settings.saved_tokens.push(SavedToken {
                                        name: token_name.clone(),
                                        token: new_token.clone(),
                                    });
                                    
                                    // Also set as current token
                                    settings.history_token = new_token.clone();
                                    
                                    if let Err(e) = settings.store(&config_path) {
                                        log::error!("Failed to save new token: {}", e);
                                        *STATE.token_generation_error.lock().unwrap() = format!("Failed to save: {}", e);
                                    } else {
                                        log::info!("Token '{}' generated and saved successfully", token_name);
                                        
                                        // Apply the token to the UI
                                        *STATE.generated_token.lock().unwrap() = new_token;
                                        
                                        // Show success message
                                        *STATE.token_validation_message.lock().unwrap() = 
                                            format!("Token '{}' created successfully!", token_name);
                                        *STATE.token_validation_is_error.lock().unwrap() = false;
                                        *STATE.token_validation_message_until.lock().unwrap() = 
                                            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                                        
                                        // Signal to close the modal on next frame (using global STATE so it works across threads!)
                                        *STATE.token_modal_should_close.lock().unwrap() = true;
                                    }
                                    
                                    *STATE.token_generating.lock().unwrap() = false;
                                }
                                Err(e) => {
                                    log::error!("Failed to generate token: {}", e);
                                    *STATE.token_generation_error.lock().unwrap() = format!("Failed: {}", e);
                                    *STATE.token_generating.lock().unwrap() = false;
                                }
                            }
                        });
                    }
                }
            } else if is_generating {
                let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
                let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
                let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
                ui.button("Generating...");
            } else {
                let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
                let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
                let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
                ui.button("Generate & Save");
            }
            
            ui.same_line();
            
            if !is_generating && ui.button("Cancel") {
                log::info!("Cancel button clicked - closing modal");
                SHOW_NAME_MODAL.set(false);
                STATE.token_generation_error.lock().unwrap().clear();
                DUPLICATE_NAME_ERROR.set(String::new());
                ui.close_current_popup();
                POPUP_JUST_OPENED.set(false);
            }
        });
}
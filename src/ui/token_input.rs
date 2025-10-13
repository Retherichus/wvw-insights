use nexus::imgui::Ui;

use crate::scanning::scan_for_logs;
use crate::settings::Settings;
use crate::state::STATE;
use crate::tokens::{generate_token, validate_token};

/// Renders the token input screen
pub fn render_token_input(ui: &Ui, config_path: &std::path::Path) {
    thread_local! {
        static TOKEN_BUFFER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static INITIALIZED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static LAST_LOADED_TOKEN: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    }

    // Check if we need to reload from settings (e.g., token was changed in settings)
    let current_settings_token = Settings::get().history_token.clone();
    let should_reload = LAST_LOADED_TOKEN.with_borrow(|last| last != &current_settings_token);
    
    if !INITIALIZED.get() || should_reload {
        TOKEN_BUFFER.set(current_settings_token.clone());
        LAST_LOADED_TOKEN.set(current_settings_token);
        INITIALIZED.set(true);
    }

    // Check if we have a newly generated token to insert
    let generated_token = STATE.generated_token.lock().unwrap();
    if !generated_token.is_empty() {
        TOKEN_BUFFER.set(generated_token.clone());
        drop(generated_token);
        STATE.generated_token.lock().unwrap().clear();
    } else {
        drop(generated_token);
    }

    ui.text("Enter your History Token");
    ui.spacing();

    TOKEN_BUFFER.with_borrow_mut(|token| {
        ui.input_text("##token", token).build();
    });

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
            
            // Clone config_path for the thread
            let config_path = config_path.to_path_buf();
            
            // Start validation
            *STATE.token_validating.lock().unwrap() = true;
            STATE.token_validation_message.lock().unwrap().clear();
            *STATE.token_validation_message_until.lock().unwrap() = None;
            
            std::thread::spawn(move || {
                log::info!("Validating token...");
                
                match validate_token(&api_endpoint, &token_to_validate) {
                    Ok(true) => {
                        log::info!("Token validation successful");
                        
                        // Save the token
                        let mut settings = Settings::get();
                        settings.history_token = token_to_validate.clone();
                        
                        if let Err(e) = settings.store(&config_path) {
                            log::error!("Failed to save config: {}", e);
                        }
                        drop(settings);
                        
                        // Scan for logs
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
    
    if ui.button("Settings") {
        *STATE.show_token_input.lock().unwrap() = false;
        *STATE.show_settings.lock().unwrap() = true;
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // Generate key button - only enabled if token field is empty and not currently generating
    let button_enabled = token_is_empty && !is_generating;
    
    if button_enabled {
        if ui.button("Generate Key") {
            log::info!("Generate Key button clicked");
            *STATE.token_generating.lock().unwrap() = true;
            STATE.token_generation_error.lock().unwrap().clear();
            
            std::thread::spawn(|| {
                log::info!("Generating new token from server");
                
                match generate_token() {
                    Ok(new_token) => {
                        log::info!("Token generated successfully: {}", new_token);
                        *STATE.generated_token.lock().unwrap() = new_token;
                        *STATE.token_generating.lock().unwrap() = false;
                    }
                    Err(e) => {
                        log::error!("Failed to generate token: {}", e);
                        *STATE.token_generation_error.lock().unwrap() = format!("Failed to generate token: {}", e);
                        *STATE.token_generating.lock().unwrap() = false;
                    }
                }
            });
        }
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Generate Key");
    }
    
    if !token_is_empty && !is_generating {
        ui.same_line();
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "(Clear token field to generate new key)");
    }
}
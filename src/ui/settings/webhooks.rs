use nexus::imgui::Ui;
use std::cell::RefCell;
use crate::webhooks::WebhookSettings;

thread_local! {
    static WEBHOOK_NAME_BUFFER: std::cell::RefCell<String> = RefCell::new(String::new());
    static WEBHOOK_URL_BUFFER: std::cell::RefCell<String> = RefCell::new(String::new());
    static STATUS_MESSAGE: std::cell::RefCell<String> = RefCell::new(String::new());
    static STATUS_MESSAGE_UNTIL: std::cell::Cell<Option<std::time::Instant>> = std::cell::Cell::new(None);
    static STATUS_IS_ERROR: std::cell::Cell<bool> = std::cell::Cell::new(false);
    static DELETE_CONFIRM_WEBHOOK: std::cell::RefCell<String> = RefCell::new(String::new());
}

pub fn render_webhooks_tab(ui: &Ui, _config_path: &std::path::Path) {
    ui.text("Discord Webhook Manager");
    ui.text_colored([0.7, 0.7, 0.7, 1.0], "Manage your saved Discord webhooks for posting reports");
    
    ui.spacing();
    ui.separator();
    ui.spacing();

    // Show temporary status message
    let message_until = STATUS_MESSAGE_UNTIL.get();
    if let Some(until) = message_until {
        if std::time::Instant::now() < until {
            STATUS_MESSAGE.with(|msg| {
                let msg_str = msg.borrow();
                if !msg_str.is_empty() {
                    let is_error = STATUS_IS_ERROR.get();
                    let color = if is_error {
                        [1.0, 0.5, 0.0, 1.0]
                    } else {
                        [0.0, 1.0, 0.0, 1.0]
                    };
                    ui.text_colored(color, &*msg_str);
                    ui.spacing();
                }
            });
        } else {
            STATUS_MESSAGE_UNTIL.set(None);
            STATUS_MESSAGE.with(|msg| msg.borrow_mut().clear());
        }
    }

    // Add new webhook section
    ui.text("Add New Webhook:");
    ui.spacing();
    
    // Label for name field
    ui.text("Webhook Name:");
    WEBHOOK_NAME_BUFFER.with(|name| {
        let mut name_mut = name.borrow_mut();
        ui.input_text("##webhook_name", &mut *name_mut)
            .hint("e.g., 'Main Guild', 'WvW Squad'")
            .build();
    });

    ui.spacing();
    
    // Label for URL field
    ui.text("Webhook URL:");
    WEBHOOK_URL_BUFFER.with(|url| {
        let mut url_mut = url.borrow_mut();
        ui.input_text("##webhook_url", &mut *url_mut)
            .hint("https://discord.com/api/webhooks/...")
            .build();
    });

    ui.spacing();

    if ui.button("Save Webhook") {
        // Get values without holding borrows
        let name = WEBHOOK_NAME_BUFFER.with(|n| n.borrow().trim().to_string());
        let url = WEBHOOK_URL_BUFFER.with(|u| u.borrow().trim().to_string());

        if name.is_empty() {
            show_message("Please enter a webhook name", true);
        } else if url.is_empty() {
            show_message("Please enter a webhook URL", true);
        } else if !url.starts_with("https://discord.com/api/webhooks/") 
            && !url.starts_with("https://discordapp.com/api/webhooks/") {
            show_message("Invalid Discord webhook URL", true);
        } else {
            let mut webhook_settings = WebhookSettings::get();
            match webhook_settings.add_webhook(name, url) {
                Ok(_) => {
                    if let Err(e) = webhook_settings.store(crate::webhooks_path()) {
                        log::error!("Failed to save webhook settings: {}", e);
                        show_message("Failed to save webhook", true);
                    } else {
                        show_message("Webhook saved successfully!", false);
                        WEBHOOK_NAME_BUFFER.with(|n| n.borrow_mut().clear());
                        WEBHOOK_URL_BUFFER.with(|u| u.borrow_mut().clear());
                    }
                }
                Err(e) => {
                    show_message(&e, true);
                }
            }
        }
    }

    ui.same_line();
    
    ui.text_colored([0.5, 0.5, 1.0, 1.0], "(?)");
    if ui.is_item_hovered() {
        ui.tooltip_text("How to get a Discord webhook:\n1. Go to your Discord server\n2. Edit channel → Integrations → Webhooks\n3. Create a new webhook\n4. Copy the webhook URL");
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    // Saved webhooks list
    ui.text("Saved Webhooks:");
    
    let webhook_settings = WebhookSettings::get();
    let webhooks = webhook_settings.get_webhooks_sorted();
    drop(webhook_settings);

    if webhooks.is_empty() {
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "No saved webhooks yet.");
    } else {
        for webhook in webhooks.iter() {
            ui.spacing();
            
            // Webhook name
            ui.text(&webhook.name);
            
            // URL preview
            let url_preview = if webhook.url.len() > 50 {
                format!("{}...", &webhook.url[..50])
            } else {
                webhook.url.clone()
            };
            ui.text_colored([0.6, 0.6, 0.6, 1.0], &url_preview);
            
            // Last used
            let last_used = format_timestamp(webhook.last_used);
            ui.text_colored([0.5, 0.5, 0.5, 1.0], &format!("Last used: {}", last_used));
            
            // Delete button
            let delete_id = format!("Delete##{}", webhook.name);
            if ui.button(&delete_id) {
                DELETE_CONFIRM_WEBHOOK.with(|w| *w.borrow_mut() = webhook.name.clone());
                ui.open_popup("delete_webhook_confirm");
            }
            
            ui.separator();
        }
    }

    // Delete confirmation popup
    ui.popup_modal("delete_webhook_confirm")
        .always_auto_resize(true)
        .build(ui, || {
            DELETE_CONFIRM_WEBHOOK.with(|webhook_name_cell| {
                let webhook_name = webhook_name_cell.borrow();
                ui.text(&format!("Delete webhook '{}'?", webhook_name));
                ui.spacing();
                ui.text_colored([1.0, 1.0, 0.0, 1.0], "This action cannot be undone.");
                ui.spacing();

                if ui.button("Yes, Delete") {
                    let name_to_delete = webhook_name.clone();
                    drop(webhook_name);
                    
                    let mut webhook_settings = WebhookSettings::get();
                    if webhook_settings.delete_webhook(&name_to_delete) {
                        if let Err(e) = webhook_settings.store(crate::webhooks_path()) {
                            log::error!("Failed to save webhook settings: {}", e);
                        } else {
                            show_message("Webhook deleted successfully!", false);
                        }
                    }
                    ui.close_current_popup();
                    DELETE_CONFIRM_WEBHOOK.with(|w| w.borrow_mut().clear());
                }

                ui.same_line();

                if ui.button("Cancel") {
                    ui.close_current_popup();
                    DELETE_CONFIRM_WEBHOOK.with(|w| w.borrow_mut().clear());
                }
            });
        });
}

fn show_message(message: &str, is_error: bool) {
    STATUS_MESSAGE.with(|msg| *msg.borrow_mut() = message.to_string());
    STATUS_IS_ERROR.set(is_error);
    STATUS_MESSAGE_UNTIL.set(Some(std::time::Instant::now() + std::time::Duration::from_secs(3)));
}

fn format_timestamp(timestamp: u64) -> String {
    use chrono::{DateTime, Local, Utc};
    
    let datetime = DateTime::<Utc>::from_timestamp(timestamp as i64, 0)
        .unwrap_or_else(|| Utc::now());
    let local: DateTime<Local> = datetime.into();
    
    let now = Local::now();
    let diff = now.signed_duration_since(local);
    
    if diff.num_days() == 0 {
        "Today".to_string()
    } else if diff.num_days() == 1 {
        "Yesterday".to_string()
    } else if diff.num_days() < 7 {
        format!("{} days ago", diff.num_days())
    } else {
        local.format("%Y-%m-%d").to_string()
    }
}
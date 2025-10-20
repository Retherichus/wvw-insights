use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedWebhook {
    pub name: String,
    pub url: String,
    pub created: u64,      // Unix timestamp
    pub last_used: u64,    // Unix timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSettings {
    #[serde(default)]
    pub saved_webhooks: Vec<SavedWebhook>,
    #[serde(default)]
    pub remember_last_webhook: bool,
    #[serde(default)]
    pub last_webhook_url: String,
}

impl WebhookSettings {
    const fn default() -> Self {
        Self {
            saved_webhooks: Vec::new(),
            remember_last_webhook: false,
            last_webhook_url: String::new(),
        }
    }

    pub fn init(&mut self) {
        self.saved_webhooks = Vec::new();
        self.remember_last_webhook = false;
        self.last_webhook_url = String::new();
    }

    pub fn get() -> MutexGuard<'static, Self> {
        WEBHOOK_SETTINGS.lock().unwrap()
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        log::info!("Loading webhook settings from: {:?}", path);
        
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let settings: Self = serde_json::from_str(&contents)?;
            *WEBHOOK_SETTINGS.lock().unwrap() = settings;
            log::info!("Loaded {} saved webhooks", WEBHOOK_SETTINGS.lock().unwrap().saved_webhooks.len());
        } else {
            log::info!("Webhook settings file doesn't exist, initializing defaults");
            let mut settings = WEBHOOK_SETTINGS.lock().unwrap();
            settings.init();
        }
        Ok(())
    }

    pub fn store(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let prefix = path.parent().unwrap();
        create_dir_all(prefix)?;
        let mut file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        Ok(())
    }

    pub fn add_webhook(&mut self, name: String, url: String) -> Result<(), String> {
        // Check for duplicate URL
        if self.saved_webhooks.iter().any(|w| w.url == url) {
            return Err("This webhook URL is already saved".to_string());
        }

        // Check for duplicate name
        if self.saved_webhooks.iter().any(|w| w.name == name) {
            return Err("A webhook with this name already exists".to_string());
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.saved_webhooks.push(SavedWebhook {
            name,
            url,
            created: timestamp,
            last_used: timestamp,
        });

        Ok(())
    }

    pub fn delete_webhook(&mut self, name: &str) -> bool {
        let initial_len = self.saved_webhooks.len();
        self.saved_webhooks.retain(|w| w.name != name);
        self.saved_webhooks.len() < initial_len
    }

    pub fn update_webhook_usage(&mut self, url: &str) {
        if let Some(webhook) = self.saved_webhooks.iter_mut().find(|w| w.url == url) {
            webhook.last_used = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
    }

    pub fn get_webhooks_sorted(&self) -> Vec<SavedWebhook> {
        let mut webhooks = self.saved_webhooks.clone();
        webhooks.sort_by(|a, b| b.last_used.cmp(&a.last_used));
        webhooks
    }
}

static WEBHOOK_SETTINGS: Mutex<WebhookSettings> = Mutex::new(WebhookSettings::default());

/// Validates a Discord webhook URL
fn validate_webhook_url(webhook_url: &str) -> Result<()> {
    // Check if URL is empty
    if webhook_url.trim().is_empty() {
        return Err(anyhow::anyhow!("Webhook URL cannot be empty"));
    }

    // Check if URL starts with valid Discord webhook prefix
    if !webhook_url.starts_with("https://discord.com/api/webhooks/") 
        && !webhook_url.starts_with("https://discordapp.com/api/webhooks/") {
        return Err(anyhow::anyhow!("Invalid Discord webhook URL format"));
    }

    // Additional validation: check URL has parts after the prefix
    let parts: Vec<&str> = webhook_url.split('/').collect();
    if parts.len() < 7 {
        // Expected: https / / discord.com / api / webhooks / ID / TOKEN
        return Err(anyhow::anyhow!("Incomplete Discord webhook URL"));
    }

    // Check that the webhook ID and token parts are not empty
    if parts.get(5).map_or(true, |s| s.is_empty()) || parts.get(6).map_or(true, |s| s.is_empty()) {
        return Err(anyhow::anyhow!("Discord webhook URL is missing ID or token"));
    }

    Ok(())
}

/// Send a message to a Discord webhook
pub fn send_to_discord(webhook_url: &str, message_content: &str) -> Result<()> {
    // Validate the webhook URL first
    validate_webhook_url(webhook_url)?;

    // Validate message content
    if message_content.trim().is_empty() {
        return Err(anyhow::anyhow!("Message content cannot be empty"));
    }

    let payload = serde_json::json!({
        "content": message_content,
        "username": "WvW Insights Parser",
        "avatar_url": "https://parser.rethl.net/Assets/Avatar.png"
    });

    // Send the HTTP request with proper error handling
    let response = match ureq::post(webhook_url)
        .set("Content-Type", "application/json")
        .send_json(&payload) {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("Failed to send webhook request: {}", e);
            return Err(anyhow::anyhow!("Failed to send webhook request: {}", e));
        }
    };

    // Discord returns 204 No Content on success
    if response.status() == 204 || response.status() == 200 {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Discord returned status: {}", response.status()))
    }
}
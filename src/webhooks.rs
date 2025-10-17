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

/// Send a message to a Discord webhook
pub fn send_to_discord(webhook_url: &str, message_content: &str) -> Result<()> {
    let payload = serde_json::json!({
        "content": message_content,
        "username": "WvW Insights Parser",
        "avatar_url": "https://parser.rethl.net/Assets/Avatar.png"
    });

    let response = ureq::post(webhook_url)
        .set("Content-Type", "application/json")
        .send_json(&payload)?;

    // Discord returns 204 No Content on success
    if response.status() == 204 || response.status() == 200 {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Discord returned status: {}", response.status()))
    }
}
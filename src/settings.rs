use anyhow::Result;
use dirs_next::document_dir;
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedToken {
    pub name: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportHistoryEntry {
    pub url: String,
    pub timestamp: u64, // Unix timestamp
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub history_token: String,
    pub api_endpoint: String,
    pub log_directory: String,
    #[serde(default = "default_show_formatted_timestamps")]
    pub show_formatted_timestamps: bool,
    #[serde(default)]
    pub saved_tokens: Vec<SavedToken>,
    #[serde(default)]
    pub report_history: Vec<ReportHistoryEntry>,
    #[serde(default)]
    pub auto_cleanup_enabled: bool,
    #[serde(default = "default_cleanup_days")]
    pub auto_cleanup_days: u32,
}

fn default_cleanup_days() -> u32 {
    30
}

fn default_show_formatted_timestamps() -> bool {
    true // Default to the prettier format
}

impl Settings {
    const fn default() -> Self {
        Self {
            history_token: String::new(),
            api_endpoint: String::new(),
            log_directory: String::new(),
            show_formatted_timestamps: true,
            saved_tokens: Vec::new(),
            report_history: Vec::new(),
            auto_cleanup_enabled: false,
            auto_cleanup_days: 30,
        }
    }

    pub fn init(&mut self) {
        self.api_endpoint = "https://parser.rethl.net/api.php".to_string();
        self.log_directory = Self::default_log_dir().display().to_string();
        self.show_formatted_timestamps = true;
        self.auto_cleanup_enabled = false;
        self.auto_cleanup_days = 30;
    }

    pub fn get() -> MutexGuard<'static, Self> {
        SETTINGS.lock().unwrap()
    }

    pub fn default_log_dir() -> PathBuf {
        let mut base = document_dir().unwrap_or_default();
        base.push("Guild Wars 2");
        base.push("addons");
        base.push("arcdps");
        base.push("arcdps.cbtlogs");
        base
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let settings: Self = serde_json::from_str(&contents)?;
            *SETTINGS.lock().unwrap() = settings;
        } else {
            let mut settings = SETTINGS.lock().unwrap();
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
}

static SETTINGS: Mutex<Settings> = Mutex::new(Settings::default());

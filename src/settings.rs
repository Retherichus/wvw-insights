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
pub struct Settings {
    pub history_token: String,
    pub api_endpoint: String,
    pub log_directory: String,
    #[serde(default = "default_show_formatted_timestamps")]
    pub show_formatted_timestamps: bool,
    #[serde(default)]
    pub saved_tokens: Vec<SavedToken>,
    #[serde(default)]
    pub auto_cleanup_enabled: bool,
    #[serde(default = "default_cleanup_days")]
    pub auto_cleanup_days: u32,
    #[serde(default)]
    pub mouse_lock_enabled: bool,
    #[serde(default)]
    pub guild_name: String,
    #[serde(default)]
    pub enable_legacy_parser: bool,
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
            auto_cleanup_enabled: false,
            auto_cleanup_days: 30,
            mouse_lock_enabled: false,
            guild_name: String::new(),
            enable_legacy_parser: false,
        }
    }


    pub fn init(&mut self) {
        self.api_endpoint = "https://parser.rethl.net/api.php".to_string();
        self.log_directory = Self::default_log_dir().display().to_string();
        self.show_formatted_timestamps = true;
        self.auto_cleanup_enabled = false;
        self.auto_cleanup_days = 30;
        self.mouse_lock_enabled = false;
        self.guild_name = String::new();
        self.enable_legacy_parser = false;
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
        log::info!("Loading settings from: {:?}", path);
        
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            log::info!("Settings file contents: {}", contents);
            let mut settings: Self = serde_json::from_str(&contents)?;
            
            // Fix empty API endpoint
            if settings.api_endpoint.is_empty() {
                log::warn!("API endpoint was empty, setting to default");
                settings.api_endpoint = "https://parser.rethl.net/api.php".to_string();
            }
            
            // Auto-sync with ArcDPS if log directory is empty
            if settings.log_directory.is_empty() {
                log::info!("Log directory is empty, attempting to sync with ArcDPS...");
                match crate::arcdps::sync_with_arcdps() {
                    Ok(arcdps_path) => {
                        log::info!("Auto-synced log directory from ArcDPS: {}", arcdps_path);
                        settings.log_directory = arcdps_path;
                    }
                    Err(e) => {
                        log::warn!("Could not auto-sync with ArcDPS: {}, using default", e);
                        settings.log_directory = Self::default_log_dir().display().to_string();
                    }
                }
            }
            
            log::info!("Parsed settings - log_directory: '{}'", settings.log_directory);
            *SETTINGS.lock().unwrap() = settings;
        } else {
            log::info!("Settings file doesn't exist, initializing defaults");
            let mut settings = SETTINGS.lock().unwrap();
            settings.init();
            
            // Try to auto-sync with ArcDPS on first launch
            log::info!("First launch - attempting to sync with ArcDPS...");
            drop(settings); // Drop the lock before calling sync
            
            match crate::arcdps::sync_with_arcdps() {
                Ok(arcdps_path) => {
                    log::info!("Auto-synced log directory from ArcDPS: {}", arcdps_path);
                    let mut settings = SETTINGS.lock().unwrap();
                    settings.log_directory = arcdps_path;
                }
                Err(e) => {
                    log::warn!("Could not auto-sync with ArcDPS: {}, using default", e);
                    // Keep the default from init()
                }
            }
            
            log::info!("Initialized settings - log_directory: '{}'", SETTINGS.lock().unwrap().log_directory);
            // Save the initialized settings
            let settings = SETTINGS.lock().unwrap();
            settings.store(path)?;
            log::info!("Saved initialized settings to disk");
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
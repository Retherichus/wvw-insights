use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{create_dir_all, File};
use std::path::Path;
use std::sync::{LazyLock, Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedLogs {
    pub filenames: HashSet<String>,
}

impl UploadedLogs {
    pub fn get() -> MutexGuard<'static, Self> {
        UPLOADED_LOGS.lock().unwrap()
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        log::info!("Loading uploaded logs from: {:?}", path);
        
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let uploaded: Self = serde_json::from_str(&contents)?;
            log::info!("Loaded {} previously uploaded logs", uploaded.filenames.len());
            *UPLOADED_LOGS.lock().unwrap() = uploaded;
        } else {
            log::info!("No uploaded logs file exists yet, starting fresh");
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
        log::info!("Saved {} uploaded logs to disk", self.filenames.len());
        Ok(())
    }

    pub fn add_log(&mut self, filename: String) {
        self.filenames.insert(filename);
    }

    pub fn is_uploaded(&self, filename: &str) -> bool {
        self.filenames.contains(filename)
    }

    pub fn clear(&mut self) {
        self.filenames.clear();
    }
}

// Use LazyLock to lazily initialize the static
static UPLOADED_LOGS: LazyLock<Mutex<UploadedLogs>> = LazyLock::new(|| {
    Mutex::new(UploadedLogs {
        filenames: HashSet::new(),
    })
});
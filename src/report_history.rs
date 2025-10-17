use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEntry {
    pub session_id: String,
    pub timestamp: u64,
    pub main_report_url: String,
    pub legacy_report_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReportHistory {
    pub reports: Vec<ReportEntry>,
}

impl ReportHistory {
    pub fn get() -> MutexGuard<'static, Self> {
        REPORT_HISTORY.lock().unwrap()
    }

    /// Add a new report session with main and optional legacy URLs
    pub fn add_report(
        &mut self,
        session_id: String,
        timestamp: u64,
        main_url: String,
        legacy_url: Option<String>,
    ) {
        self.reports.push(ReportEntry {
            session_id,
            timestamp,
            main_report_url: main_url,
            legacy_report_url: legacy_url,
        });
    }

    /// Remove a report by index
    pub fn remove_report(&mut self, index: usize) {
        if index < self.reports.len() {
            self.reports.remove(index);
        }
    }

    /// Clear all reports
    pub fn clear(&mut self) {
        self.reports.clear();
    }

    /// Load from file
    pub fn from_path(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let history: Self = serde_json::from_str(&contents)?;
            let count = history.reports.len();
            *REPORT_HISTORY.lock().unwrap() = history;
            log::info!("Loaded {} reports from history", count);
        } else {
            log::info!("Report history file doesn't exist yet");
        }
        Ok(())
    }

    /// Save to file
    pub fn store(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(prefix) = path.parent() {
            create_dir_all(prefix)?;
        }
        let mut file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        Ok(())
    }
}

static REPORT_HISTORY: Mutex<ReportHistory> = Mutex::new(ReportHistory {
    reports: Vec::new(),
});
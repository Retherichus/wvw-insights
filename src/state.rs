use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use std::thread;

use crate::common::WorkerMessage;
use crate::upload_review::UploadedFileInfo;
use crate::logfile::LogFile;
use crate::upload;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessingState {
    Idle,
    Uploading,
    Processing,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeFilter {
    SincePluginStart,
    Last24Hours,
    Last48Hours,
    Last72Hours,
}

pub struct State {
    // ============================================
    // Worker Threads & Communication
    // ============================================
    pub upload_worker: Mutex<Option<Sender<upload::UploadJob>>>,
    pub producer_rx: Mutex<Option<Receiver<WorkerMessage>>>,
    pub threads: Mutex<Vec<thread::JoinHandle<()>>>,

    // ============================================
    // Log Management
    // ============================================
    pub logs: Mutex<Vec<LogFile>>,
    pub selected_time_filter: Mutex<TimeFilter>,
    pub last_auto_scan: Mutex<Option<std::time::Instant>>,
    pub last_scan_display: Mutex<String>,
    pub current_scan_id: Mutex<u64>,
    pub scan_in_progress: Mutex<bool>,

    // ============================================
    // Upload & Processing State
    // ============================================
    pub session_id: Mutex<String>,
    pub ownership_token: Mutex<String>,
    pub report_urls: Mutex<Vec<String>>,
    pub processing_state: Mutex<ProcessingState>,
    pub last_status_check: Mutex<Option<std::time::Instant>>,
    pub processing_progress: Mutex<f32>,
    pub processing_phase: Mutex<String>,
    pub uploaded_files: Mutex<Vec<UploadedFileInfo>>,
    pub processing_time_estimate: Mutex<Option<u32>>,
    pub processing_time_estimate_start: Mutex<Option<std::time::Instant>>,

    // ============================================
    // UI Window Visibility
    // ============================================
    pub show_main_window: Mutex<bool>,
    pub show_token_input: Mutex<bool>,
    pub show_log_selection: Mutex<bool>,
    pub show_upload_progress: Mutex<bool>,
    pub show_results: Mutex<bool>,
    pub show_settings: Mutex<bool>,
    #[allow(dead_code)]
    pub show_recent_logs: Mutex<bool>,
    pub show_uploaded_logs: Mutex<bool>,
    pub show_upload_review: Mutex<bool>,

    // ============================================
    // Token Generation (Main Page)
    // ============================================
    pub generated_token: Mutex<String>,
    pub token_generating: Mutex<bool>,
    pub token_generation_error: Mutex<String>,

    // ============================================
    // Token Validation (Main Page - Continue Button)
    // ============================================
    pub token_validating: Mutex<bool>,
    pub token_validation_message: Mutex<String>,
    pub token_validation_message_until: Mutex<Option<std::time::Instant>>,
    pub token_validation_is_error: Mutex<bool>,

    // ============================================
    // Token Manager (Settings Page)
    // ============================================
    pub token_applied_message: Mutex<String>,
    pub token_applied_message_until: Mutex<Option<std::time::Instant>>,
    pub save_token_validating: Mutex<bool>,
    pub save_token_validation_message: Mutex<String>,
    pub save_token_validation_message_until: Mutex<Option<std::time::Instant>>,
    pub save_token_validation_is_error: Mutex<bool>,

    // ============================================
    // ArcDPS Integration
    // ============================================
    pub sync_arcdps_result: Mutex<Option<Result<String, String>>>,
    pub sync_arcdps_pending: Mutex<bool>,
    pub sync_arcdps_message: Mutex<String>,
    pub sync_arcdps_message_until: Mutex<Option<std::time::Instant>>,
    pub sync_arcdps_message_is_error: Mutex<bool>,

    // ============================================
    // Cleanup Operations
    // ============================================
    pub cleanup_in_progress: Mutex<bool>,
    pub cleanup_result: Mutex<Option<Result<(usize, u64), String>>>,
    pub cleanup_message_until: Mutex<Option<std::time::Instant>>,
    pub auto_cleanup_done: Mutex<bool>,

    // ============================================
    // UI Resources & Misc
    // ============================================
    pub icon_texture: Mutex<Option<&'static nexus::texture::Texture>>,
    pub icon_hover_texture: Mutex<Option<&'static nexus::texture::Texture>>,
    pub addon_load_time: Mutex<Option<std::time::Instant>>,
    
    // ============================================
    // Discord Webhook
    // ============================================
    pub show_webhook_modal: Mutex<bool>,
    pub webhook_url_input: Mutex<String>,
    pub webhook_remember: Mutex<bool>,
    pub webhook_sending: Mutex<bool>,
    pub webhook_status_message: Mutex<String>,
    pub webhook_status_until: Mutex<Option<std::time::Instant>>,
    pub webhook_status_is_error: Mutex<bool>,
    pub webhook_selected_name: Mutex<String>,
}

impl State {
    pub fn try_next_producer(&self) -> Option<WorkerMessage> {
        let guard = self.producer_rx.lock().unwrap();
        guard.as_ref().and_then(|rx| rx.try_recv().ok())
    }

    pub fn init_producer(&self) -> Sender<WorkerMessage> {
        let (tx, rx) = mpsc::channel();
        *self.producer_rx.lock().unwrap() = Some(rx);
        tx
    }

    pub fn init_upload_worker(&self) -> Receiver<upload::UploadJob> {
        let (tx, rx) = mpsc::channel();
        *self.upload_worker.lock().unwrap() = Some(tx);
        rx
    }

    pub fn append_thread(&self, handle: thread::JoinHandle<()>) {
        self.threads.lock().unwrap().push(handle);
    }
}

pub static STATE: State = State {
    // ============================================
    // Worker Threads & Communication
    // ============================================
    upload_worker: Mutex::new(None),
    producer_rx: Mutex::new(None),
    threads: Mutex::new(Vec::new()),

    // ============================================
    // Log Management
    // ============================================
    logs: Mutex::new(Vec::new()),
    selected_time_filter: Mutex::new(TimeFilter::SincePluginStart),
    last_auto_scan: Mutex::new(None),
    last_scan_display: Mutex::new(String::new()),
    current_scan_id: Mutex::new(0),
    scan_in_progress: Mutex::new(false), 

    // ============================================
    // Upload & Processing State
    // ============================================
    session_id: Mutex::new(String::new()),
    ownership_token: Mutex::new(String::new()),
    report_urls: Mutex::new(Vec::new()),
    processing_state: Mutex::new(ProcessingState::Idle),
    last_status_check: Mutex::new(None),
    processing_progress: Mutex::new(0.0),
    processing_phase: Mutex::new(String::new()),
    uploaded_files: Mutex::new(Vec::new()),
    processing_time_estimate: Mutex::new(None),
    processing_time_estimate_start: Mutex::new(None),

    // ============================================
    // UI Window Visibility
    // ============================================
    show_main_window: Mutex::new(false),
    show_token_input: Mutex::new(true),
    show_log_selection: Mutex::new(false),
    show_upload_progress: Mutex::new(false),
    show_results: Mutex::new(false),
    show_settings: Mutex::new(false),
    show_recent_logs: Mutex::new(true),
    show_uploaded_logs: Mutex::new(true),
    show_upload_review: Mutex::new(false),

    // ============================================
    // Token Generation (Main Page)
    // ============================================
    generated_token: Mutex::new(String::new()),
    token_generating: Mutex::new(false),
    token_generation_error: Mutex::new(String::new()),

    // ============================================
    // Token Validation (Main Page - Continue Button)
    // ============================================
    token_validating: Mutex::new(false),
    token_validation_message: Mutex::new(String::new()),
    token_validation_message_until: Mutex::new(None),
    token_validation_is_error: Mutex::new(false),

    // ============================================
    // Token Manager (Settings Page)
    // ============================================
    token_applied_message: Mutex::new(String::new()),
    token_applied_message_until: Mutex::new(None),
    save_token_validating: Mutex::new(false),
    save_token_validation_message: Mutex::new(String::new()),
    save_token_validation_message_until: Mutex::new(None),
    save_token_validation_is_error: Mutex::new(false),

    // ============================================
    // ArcDPS Integration
    // ============================================
    sync_arcdps_result: Mutex::new(None),
    sync_arcdps_pending: Mutex::new(false),
    sync_arcdps_message: Mutex::new(String::new()),
    sync_arcdps_message_until: Mutex::new(None),
    sync_arcdps_message_is_error: Mutex::new(false),

    // ============================================
    // Cleanup Operations
    // ============================================
    cleanup_in_progress: Mutex::new(false),
    cleanup_result: Mutex::new(None),
    cleanup_message_until: Mutex::new(None),
    auto_cleanup_done: Mutex::new(false),

    // ============================================
    // UI Resources & Misc
    // ============================================
    icon_texture: Mutex::new(None),
    icon_hover_texture: Mutex::new(None),
    addon_load_time: Mutex::new(None),
    
    // ============================================
    // Discord Webhook
    // ============================================
    show_webhook_modal: Mutex::new(false),
    webhook_url_input: Mutex::new(String::new()),
    webhook_remember: Mutex::new(false),
    webhook_sending: Mutex::new(false),
    webhook_status_message: Mutex::new(String::new()),
    webhook_status_until: Mutex::new(None),
    webhook_status_is_error: Mutex::new(false),
    webhook_selected_name: Mutex::new(String::new()),    
};
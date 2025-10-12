use std::{
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    thread,
    time::Duration,
};

use nexus::{
    gui::{register_render, RenderType},
    imgui::{ChildWindow, Ui, Window},
    keybind::{keybind_handler, register_keybind_with_string},
    paths::get_addon_dir,
    quick_access::{add_quick_access, add_quick_access_context_menu},
    render, texture_receive,
    texture::{load_texture_from_memory, Texture},
    AddonFlags, UpdateProvider,
};
use settings::{Settings, SavedToken, ReportHistoryEntry};
mod common;
mod logfile;
mod settings;
mod upload;

use common::*;
use logfile::LogFile;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProcessingState {
    Idle,
    Uploading,
    Processing,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TimeFilter {
    SincePluginStart,
    Last24Hours,
    Last48Hours,
    Last72Hours,
    AllLogs,
}

fn format_report_timestamp(timestamp: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let datetime = UNIX_EPOCH + std::time::Duration::from_secs(timestamp);
    let now = SystemTime::now();
    
    // Calculate time difference
    if let Ok(duration) = now.duration_since(datetime) {
        let days = duration.as_secs() / 86400;
        let hours = (duration.as_secs() % 86400) / 3600;
        let minutes = (duration.as_secs() % 3600) / 60;
        
        let relative = if days > 0 {
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        } else if hours > 0 {
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else if minutes > 0 {
            format!("{} minute{} ago", minutes, if minutes == 1 { "" } else { "s" })
        } else {
            "Just now".to_string()
        };
        
        relative
    } else {
        "Unknown".to_string()
    }
}

// Add this function after the imports and before the State struct
fn format_timestamp(filename: &str) -> Option<String> {
    // Extract timestamp from filename like "20251010-222255.zevtc"
    let parts: Vec<&str> = filename.split('-').collect();
    if parts.len() < 2 {
        return None;
    }
    
    let date_part = parts[0];
    let time_part = parts[1].split('.').next()?;
    
    if date_part.len() != 8 || time_part.len() != 6 {
        return None;
    }
    
    // Parse date: YYYYMMDD
    let year = date_part[0..4].parse::<i32>().ok()?;
    let month = date_part[4..6].parse::<u32>().ok()?;
    let day = date_part[6..8].parse::<u32>().ok()?;
    
    // Parse time: HHMMSS
    let hour = time_part[0..2].parse::<u32>().ok()?;
    let minute = time_part[2..4].parse::<u32>().ok()?;
    
    // Format month name
    let month_name = match month {
        1 => "Jan", 2 => "Feb", 3 => "Mar", 4 => "Apr",
        5 => "May", 6 => "Jun", 7 => "Jul", 8 => "Aug",
        9 => "Sep", 10 => "Oct", 11 => "Nov", 12 => "Dec",
        _ => return None,
    };
    
    // Use 24-hour format
    Some(format!(
        "{} {}, {} • {:02}:{:02}",
        month_name, day, year, hour, minute
    ))
}


struct State {
    upload_worker: Mutex<Option<Sender<upload::UploadJob>>>,
    producer_rx: Mutex<Option<Receiver<WorkerMessage>>>,
    threads: Mutex<Vec<thread::JoinHandle<()>>>,
    logs: Mutex<Vec<LogFile>>,
    session_id: Mutex<String>,
    ownership_token: Mutex<String>,
    report_url: Mutex<String>,
    processing_state: Mutex<ProcessingState>,
    last_status_check: Mutex<Option<std::time::Instant>>,
    show_token_input: Mutex<bool>,
    show_log_selection: Mutex<bool>,
    show_upload_progress: Mutex<bool>,
    show_results: Mutex<bool>,
    show_settings: Mutex<bool>,
    show_recent_logs: Mutex<bool>,
    show_main_window: Mutex<bool>,
    processing_progress: Mutex<f32>,
    processing_phase: Mutex<String>,
    generated_token: Mutex<String>,
    token_generating: Mutex<bool>,
    token_generation_error: Mutex<String>,
    icon_texture: Mutex<Option<&'static Texture>>,
    icon_hover_texture: Mutex<Option<&'static Texture>>,
    addon_load_time: Mutex<Option<std::time::Instant>>,
    selected_time_filter: Mutex<TimeFilter>,
    last_auto_scan: Mutex<Option<std::time::Instant>>,
    last_scan_display: Mutex<String>,
    sync_arcdps_result: Mutex<Option<Result<String, String>>>,
    sync_arcdps_pending: Mutex<bool>,
    sync_arcdps_message: Mutex<String>,
    sync_arcdps_message_until: Mutex<Option<std::time::Instant>>,
    sync_arcdps_message_is_error: Mutex<bool>,
    //Cleanup old logs
    cleanup_in_progress: Mutex<bool>,
    cleanup_result: Mutex<Option<Result<(usize, u64), String>>>, 
    cleanup_message_until: Mutex<Option<std::time::Instant>>,
    auto_cleanup_done: Mutex<bool>,
}

impl State {
    fn try_next_producer(&self) -> Option<WorkerMessage> {
        let guard = self.producer_rx.lock().unwrap();
        guard.as_ref().and_then(|rx| rx.try_recv().ok())
    }

    fn init_producer(&self) -> Sender<WorkerMessage> {
        let (tx, rx) = mpsc::channel();
        *self.producer_rx.lock().unwrap() = Some(rx);
        tx
    }

    fn init_upload_worker(&self) -> Receiver<upload::UploadJob> {
        let (tx, rx) = mpsc::channel();
        *self.upload_worker.lock().unwrap() = Some(tx);
        rx
    }

    fn append_thread(&self, handle: thread::JoinHandle<()>) {
        self.threads.lock().unwrap().push(handle);
    }
}

static STATE: State = State {
    upload_worker: Mutex::new(None),
    producer_rx: Mutex::new(None),
    threads: Mutex::new(Vec::new()),
    logs: Mutex::new(Vec::new()),
    session_id: Mutex::new(String::new()),
    ownership_token: Mutex::new(String::new()),
    report_url: Mutex::new(String::new()),
    processing_state: Mutex::new(ProcessingState::Idle),
    last_status_check: Mutex::new(None),
    show_token_input: Mutex::new(true),
    show_log_selection: Mutex::new(false),
    show_upload_progress: Mutex::new(false),
    show_results: Mutex::new(false),
    show_settings: Mutex::new(false),
    show_recent_logs: Mutex::new(true),
    show_main_window: Mutex::new(false),
    processing_progress: Mutex::new(0.0),
    processing_phase: Mutex::new(String::new()),
    generated_token: Mutex::new(String::new()),
    token_generating: Mutex::new(false),
    token_generation_error: Mutex::new(String::new()),
    icon_texture: Mutex::new(None),
    icon_hover_texture: Mutex::new(None),
    addon_load_time: Mutex::new(None),
    selected_time_filter: Mutex::new(TimeFilter::SincePluginStart),
    last_auto_scan: Mutex::new(None),
    last_scan_display: Mutex::new(String::new()),
    sync_arcdps_result: Mutex::new(None),
    sync_arcdps_pending: Mutex::new(false),
    sync_arcdps_message: Mutex::new(String::new()),
    sync_arcdps_message_until: Mutex::new(None),
    sync_arcdps_message_is_error: Mutex::new(false),
    // Cleanup old logs
    cleanup_in_progress: Mutex::new(false),
    cleanup_result: Mutex::new(None),
    cleanup_message_until: Mutex::new(None),
    auto_cleanup_done: Mutex::new(false),
};

fn config_path() -> PathBuf {
    get_addon_dir("wvw-insights")
        .expect("Addon dir to exist")
        .join("settings.json")
}

// Embed icon resources at compile time
const ICON_NORMAL: &[u8] = include_bytes!("Icon.png");
const ICON_HOVER: &[u8] = include_bytes!("Icon_Hover.png");

// Keybind handler to toggle window
fn handle_toggle_keybind(id: &str, is_release: bool) {
    if id == "KB_WVW_INSIGHTS_TOGGLE" && !is_release {
        let mut show = STATE.show_main_window.lock().unwrap();
        *show = !*show;
        log::info!("Toggled WvW Insights window: {}", *show);
    }
}

// Texture receive callback
fn handle_texture_receive(id: &str, texture: Option<&Texture>) {
    match id {
        "ICON_WVW_INSIGHTS" => {
            *STATE.icon_texture.lock().unwrap() = texture.map(|t| unsafe { &*(t as *const Texture) });
            log::info!("Loaded WvW Insights icon texture");
        }
        "ICON_WVW_INSIGHTS_HOVER" => {
            *STATE.icon_hover_texture.lock().unwrap() = texture.map(|t| unsafe { &*(t as *const Texture) });
            log::info!("Loaded WvW Insights hover icon texture");
        }
        _ => {}
    }
}

// Simple shortcut render (for right-click menu on Nexus icon)
fn render_simple_shortcut(ui: &Ui) {
    let mut show = STATE.show_main_window.lock().unwrap();
    if ui.checkbox("WvW Insights", &mut *show) {
        log::info!("Toggled WvW Insights window from shortcut: {}", *show);
    }
}

fn check_auto_scan() {
    // Only auto-scan if we're in "This session" mode and on the log selection screen
    let current_filter = *STATE.selected_time_filter.lock().unwrap();
    let show_log_selection = *STATE.show_log_selection.lock().unwrap();
    let show_main_window = *STATE.show_main_window.lock().unwrap();
    
    // Only proceed if window is open AND we're on log selection screen
    if !show_main_window || !show_log_selection {
        return;
    }
    
    if current_filter == TimeFilter::SincePluginStart {
        let mut last_scan = STATE.last_auto_scan.lock().unwrap();
        let should_scan = last_scan.as_ref().map_or(true, |t| t.elapsed() >= Duration::from_secs(20));
        
        if should_scan {
            *last_scan = Some(std::time::Instant::now());
            drop(last_scan);
            log::info!("Auto-scanning for new logs (This session mode)");
            scan_for_logs();
        }
    }
}

fn update_scan_display() {
    let last_scan = STATE.last_auto_scan.lock().unwrap();
    if let Some(scan_time) = *last_scan {
        let elapsed = scan_time.elapsed().as_secs();
        let display = if elapsed < 60 {
            format!("Last refreshed: {} second{} ago", elapsed, if elapsed == 1 { "" } else { "s" })
        } else {
            let minutes = elapsed / 60;
            format!("Last refreshed: {} minute{} ago", minutes, if minutes == 1 { "" } else { "s" })
        };
        *STATE.last_scan_display.lock().unwrap() = display;
    } else {
        *STATE.last_scan_display.lock().unwrap() = "Not yet refreshed".to_string();
    }
}

// Replace scan_for_logs function
fn scan_for_logs() {
    thread::spawn(|| {
        log::info!("Starting background log scan");
        let settings = Settings::get();
        let log_dir = PathBuf::from(&settings.log_directory);
        let time_filter = *STATE.selected_time_filter.lock().unwrap();
        drop(settings);

        if !log_dir.exists() {
            log::error!("Log directory doesn't exist: {:?}", log_dir);
            let mut logs = STATE.logs.lock().unwrap();
            logs.clear();
            return;
        }

        let mut found_logs = Vec::new();

        fn scan_dir_recursive(
            dir: &std::path::Path,
            logs: &mut Vec<LogFile>,
            cutoff_time: Option<std::time::SystemTime>,
        ) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_dir() {
                            scan_dir_recursive(&entry.path(), logs, cutoff_time);
                        } else if metadata.is_file() {
                            if let Some(ext) = entry.path().extension() {
                                if ext == "zevtc" {
                                    if let Ok(log) = LogFile::new(entry.path()) {
                                        if cutoff_time.map_or(true, |cutoff| {
                                            let modified_time = std::time::SystemTime::UNIX_EPOCH
                                                + std::time::Duration::from_secs(log.modified);
                                            modified_time >= cutoff
                                        }) {
                                            logs.push(log);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let cutoff_time = match time_filter {
            TimeFilter::SincePluginStart => {
                STATE.addon_load_time.lock().unwrap().map(|load_time| {
                    std::time::SystemTime::now() - load_time.elapsed()
                })
            }
            TimeFilter::Last24Hours => Some(
                std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 60 * 60),
            ),
            TimeFilter::Last48Hours => Some(
                std::time::SystemTime::now() - std::time::Duration::from_secs(48 * 60 * 60),
            ),
            TimeFilter::Last72Hours => Some(
                std::time::SystemTime::now() - std::time::Duration::from_secs(72 * 60 * 60),
            ),
            TimeFilter::AllLogs => None,
        };

        scan_dir_recursive(&log_dir, &mut found_logs, cutoff_time);
        found_logs.sort_by(|a, b| b.modified.cmp(&a.modified));

        let filter_name = match time_filter {
            TimeFilter::SincePluginStart => "since plugin start",
            TimeFilter::Last24Hours => "24-hour",
            TimeFilter::Last48Hours => "48-hour",
            TimeFilter::Last72Hours => "72-hour",
            TimeFilter::AllLogs => "all logs",
        };

        let mut logs = STATE.logs.lock().unwrap();
        *logs = found_logs;
        log::info!("Found {} log files ({} filter)", logs.len(), filter_name);
    });
}

fn update_logs() {
    while let Some(WorkerMessage { index, payload }) = STATE.try_next_producer() {
        match payload {
            WorkerType::UploadResult(result) => {
                let mut logs = STATE.logs.lock().unwrap();
                if index < logs.len() {
                    match result {
                        Ok(status) => {
                            logs[index].status = status;
                            logs[index].uploaded = true;
                        }
                        Err(e) => {
                            logs[index].status = format!("Failed: {}", e);
                        }
                    }
                }
            }
        }
    }
}

fn render_token_input(ui: &Ui) {
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
    
    // Continue button - only enabled if token is not empty
    if !token_is_empty {
        if ui.button("Continue") {
            TOKEN_BUFFER.with_borrow(|token| {
                let mut settings = Settings::get();
                settings.history_token = token.clone();
                
                if let Err(e) = settings.store(config_path()) {
                    log::error!("Failed to save config: {}", e);
                }
            });

            scan_for_logs();
            
            *STATE.show_token_input.lock().unwrap() = false;
            *STATE.show_log_selection.lock().unwrap() = true;
        }
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
            
            thread::spawn(|| {
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

fn generate_token() -> anyhow::Result<String> {
    use serde::Deserialize;
    
    #[derive(Debug, Deserialize)]
    struct TokenResponse {
        success: bool,
        token: Option<String>,
        message: Option<String>,
    }
    
    let url = "https://parser.rethl.net/api.php?endpoint=generate-token";
    
    let response = ureq::get(url).call()?;
    let token_resp: TokenResponse = response.into_json()?;
    
    if token_resp.success {
        token_resp.token.ok_or_else(|| anyhow::anyhow!("No token in response"))
    } else {
        Err(anyhow::anyhow!("Token generation failed: {}", token_resp.message.unwrap_or_default()))
    }
}

fn render_log_selection(ui: &Ui) {
    let mut logs = STATE.logs.lock().unwrap();

    ui.text(format!("Select logs to upload ({} found)", logs.len()));

    // Time filter selection
    ui.spacing();
    ui.text("Show logs from:");
    ui.spacing();

    let mut current_filter = *STATE.selected_time_filter.lock().unwrap();
    let filter_changed = {
        let mut changed = false;

        if ui.radio_button("This session (Since plugin was loaded)", &mut current_filter, TimeFilter::SincePluginStart)
        {
            changed = true;
        }

        if ui.radio_button("Last 24 hours", &mut current_filter, TimeFilter::Last24Hours) {
            changed = true;
        }

        if ui.radio_button("Last 48 hours", &mut current_filter, TimeFilter::Last48Hours) {
            changed = true;
        }

        if ui.radio_button("Last 72 hours", &mut current_filter, TimeFilter::Last72Hours) {
            changed = true;
        }

        // "Show Everything" radio button that triggers warning
        if ui.radio_button("Show Everything", &mut current_filter, TimeFilter::AllLogs) {
            // Don't apply change yet, show warning popup first
            ui.open_popup("load_all_warning");
        }

        changed
    };

    ui.spacing();

    // Refresh button
    if ui.button("Refresh") {
        drop(logs);
        *STATE.last_auto_scan.lock().unwrap() = Some(std::time::Instant::now());
        scan_for_logs();
        return;
    }

    // Show last refresh time for "This session" mode
    if current_filter == TimeFilter::SincePluginStart {
        ui.same_line();
        let display = STATE.last_scan_display.lock().unwrap();
        ui.text_colored([0.7, 0.7, 0.7, 1.0], &*display);
    }

    // Drop logs before the popup to avoid borrow issues
    drop(logs);

    // Warning popup for loading all logs
    ui.popup_modal("load_all_warning").build(ui, || {
        ui.text_colored([1.0, 0.5, 0.0, 1.0], "⚠️ Performance Warning");
        ui.spacing();
        ui.text_wrapped(
            "Loading all logs may cause performance issues if you have thousands of log files.",
        );
        ui.spacing();
        ui.text_wrapped("This could make the interface slow or unresponsive.");
        ui.spacing();

        if ui.button("I understand, load all logs anyway") {
            *STATE.selected_time_filter.lock().unwrap() = TimeFilter::AllLogs;
            ui.close_current_popup();
            scan_for_logs();
            return;
        }

        ui.same_line();

        if ui.button("Cancel") {
            ui.close_current_popup();
        }
    });

    // Apply filter change if any radio button was clicked (except AllLogs which uses popup)
    if filter_changed {
        *STATE.selected_time_filter.lock().unwrap() = current_filter;
        scan_for_logs();
        return;
    }

    // Re-acquire logs lock after popup
    let mut logs = STATE.logs.lock().unwrap();

    ui.separator();

    // Handle empty log list
    if logs.is_empty() {
        ui.text_colored(
            [1.0, 0.0, 0.0, 1.0],
            "No logs found with current filter!",
        );
        ui.spacing();

        if ui.button("Open Settings") {
            *STATE.show_log_selection.lock().unwrap() = false;
            *STATE.show_settings.lock().unwrap() = true;
            return;
        }

        ui.same_line();

        if ui.button("Back") {
            thread::spawn(|| {
                log::info!("Back button clicked from log selection");
                *STATE.show_log_selection.lock().unwrap() = false;
                *STATE.show_token_input.lock().unwrap() = true;
            });
            return;
        }

        return;
    }

    // Selection buttons - only show for safe filters
    let show_select_all = matches!(
        current_filter,
        TimeFilter::SincePluginStart | TimeFilter::Last24Hours
    );

    if show_select_all {
        if ui.button("Select All") {
            for log in logs.iter_mut() {
                log.selected = true;
            }
        }
        ui.same_line();
        if ui.button("Deselect All") {
            for log in logs.iter_mut() {
                log.selected = false;
            }
        }
    } else {
        // Show disabled buttons with tooltip for other filters
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Select All");
        if ui.is_item_hovered() {
            ui.tooltip_text("Only available for 'This session' and 'Last 24 hours' filters");
        }
        ui.same_line();
        ui.button("Deselect All");
    }

    ui.spacing();

    // Log list display
    ChildWindow::new("LogList")
        .size([0.0, 300.0])
        .build(ui, || {
            let settings = Settings::get();
            let use_formatted = settings.show_formatted_timestamps;
            drop(settings);
            
            for log in logs.iter_mut() {
                ui.checkbox(&format!("##checkbox_{}", log.filename), &mut log.selected);
                ui.same_line();
                
                if use_formatted {
                    // Show formatted timestamp
                    if let Some(formatted) = format_timestamp(&log.filename) {
                        ui.text(&formatted);
                        ui.same_line();
                        ui.text_colored([0.7, 0.7, 0.7, 1.0], &format!("({:.2} MB)", log.size as f64 / 1024.0 / 1024.0));
                    } else {
                        // Fallback if parsing fails
                        ui.text(&log.filename);
                        ui.same_line();
                        ui.text(format!("({:.2} MB)", log.size as f64 / 1024.0 / 1024.0));
                    }
                } else {
                    // Show raw filename
                    ui.text(&log.filename);
                    ui.same_line();
                    ui.text(format!("({:.2} MB)", log.size as f64 / 1024.0 / 1024.0));
                }
            }
        });

    ui.separator();

    let selected_count = logs.iter().filter(|l| l.selected).count();
    ui.text(format!("Selected: {} files", selected_count));

    let state = *STATE.processing_state.lock().unwrap();

    if state != ProcessingState::Idle {
        ui.text_colored([1.0, 1.0, 0.0, 1.0], "Upload in progress...");
        return;
    }

    if ui.button("Upload Selected") && selected_count > 0 {
        log::info!("Starting upload for {} files", selected_count);

        *STATE.show_log_selection.lock().unwrap() = false;
        *STATE.show_upload_progress.lock().unwrap() = true;

        thread::spawn(|| {
            start_upload_process();
        });
    }

    ui.same_line();

    if ui.button("Back") {
        thread::spawn(|| {
            log::info!("Back button clicked from log selection");
            *STATE.show_log_selection.lock().unwrap() = false;
            *STATE.show_token_input.lock().unwrap() = true;
        });
    }
}

fn start_upload_process() {
    log::info!("Starting upload process");
    
    *STATE.processing_state.lock().unwrap() = ProcessingState::Uploading;
    
    let settings = Settings::get();
    let api_endpoint = settings.api_endpoint.clone();
    let history_token = settings.history_token.clone();
    drop(settings);

    // Create session
    log::info!("Creating session");
    let (session_id, _ownership_token) = match upload::create_session(&api_endpoint, &history_token) {
        Ok((sid, ot)) => {
            log::info!("Session created: {}", sid);
            *STATE.session_id.lock().unwrap() = sid.clone();
            *STATE.ownership_token.lock().unwrap() = ot.clone();
            (sid, ot)
        }
        Err(e) => {
            log::error!("Failed to create session: {}", e);
            *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
            return;
        }
    };

    // Get selected logs
    let selected_logs: Vec<(usize, LogFile)> = {
        let logs = STATE.logs.lock().unwrap();
        logs.iter()
            .enumerate()
            .filter(|(_, log)| log.selected)
            .map(|(i, log)| (i, log.clone()))
            .collect()
    };
    log::info!("Queueing {} logs for upload", selected_logs.len());

    // Queue uploads
    let upload_tx = STATE.upload_worker.lock().unwrap();
    if let Some(tx) = upload_tx.as_ref() {
        for (index, log) in selected_logs.iter() {
            log::info!("Queuing: {}", log.filename);
            if let Err(e) = tx.send((
                *index,
                log.path.clone(),
                api_endpoint.clone(),
                session_id.clone(),
                history_token.clone(),
            )) {
                log::error!("Failed to queue upload: {}", e);
            }
        }
    }
    log::info!("All uploads queued");
}

fn check_upload_progress() {
    let state = *STATE.processing_state.lock().unwrap();
    
    if state == ProcessingState::Uploading {
        // Check if all uploads are done
        let logs = STATE.logs.lock().unwrap();
        let selected_logs: Vec<_> = logs.iter().filter(|l| l.selected).collect();
        let total = selected_logs.len();
        let uploaded = selected_logs.iter().filter(|l| l.uploaded || l.status.starts_with("Failed")).count();
        drop(logs);
        
        if uploaded >= total && total > 0 {
            log::info!("All uploads complete ({}/{})", uploaded, total);
            *STATE.processing_state.lock().unwrap() = ProcessingState::Idle;
        }
    } else if state == ProcessingState::Processing {
        // Poll for completion every 3 seconds
        let mut last_check = STATE.last_status_check.lock().unwrap();
        let should_check = last_check.as_ref().map_or(true, |t| t.elapsed() >= Duration::from_secs(3));
        if should_check {
            *last_check = Some(std::time::Instant::now());
            drop(last_check);
            
            thread::spawn(|| {
                let settings = Settings::get();
                let api_endpoint = settings.api_endpoint.clone();
                drop(settings);
                
                let session_id = STATE.session_id.lock().unwrap().clone();
                
                match upload::check_status(&api_endpoint, &session_id) {
                    Ok((status, report_url, progress, phase)) => {
                        // Update progress and phase
                        *STATE.processing_progress.lock().unwrap() = progress;
                        if let Some(phase_msg) = phase {
                            *STATE.processing_phase.lock().unwrap() = phase_msg;
                        }
                            if status == "complete" {
                                log::info!("Processing complete!");
                                if let Some(url) = report_url {
                                    *STATE.report_url.lock().unwrap() = url.clone();
                                    
                                    // Save to report history
                                    let session_id = STATE.session_id.lock().unwrap().clone();
                                    let mut settings = Settings::get();
                                    settings.report_history.push(ReportHistoryEntry {
                                        url: url.clone(),
                                        timestamp: std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs(),
                                        session_id,
                                    });
                                    if let Err(e) = settings.store(config_path()) {
                                        log::error!("Failed to save report to history: {}", e);
                                    } else {
                                        log::info!("Saved report to history");
                                    }
                                }
                                *STATE.processing_state.lock().unwrap() = ProcessingState::Complete;
                                *STATE.show_upload_progress.lock().unwrap() = false;
                                *STATE.show_results.lock().unwrap() = true;
                            } else if status == "failed" {
                            log::error!("Processing failed");
                            *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to check status: {}", e);
                    }
                }
            });
        }
    }
}

fn render_upload_progress(ui: &Ui) {
    ui.text("Upload Progress");
    ui.separator();

    ChildWindow::new("UploadStatus")
        .size([0.0, 300.0])
        .build(ui, || {
            let logs = STATE.logs.lock().unwrap();
            for log in logs.iter() {
                if log.selected {
                    ui.text(format!("{}: {}", log.filename, log.status));
                }
            }
        });

    let state = *STATE.processing_state.lock().unwrap();
    
    ui.separator();
    
    match state {
        ProcessingState::Uploading => {
            ui.text("Uploading files...");
            ui.spacing();
            
            if ui.button("Cancel Upload") {
                ui.open_popup("cancel_upload_confirmation");
            }
            
            ui.popup_modal("cancel_upload_confirmation")
                .always_auto_resize(true)
                .build(ui, || {
                    ui.text("Are you sure you want to cancel this job?");
                    ui.spacing();
                    ui.text_colored([1.0, 1.0, 0.0, 1.0], "The upload will be abandoned.");
                    ui.spacing();

                    if ui.button("Yes, Cancel") {
                        ui.close_current_popup();
                        thread::spawn(|| {
                            log::info!("User cancelled upload");
                            reset_upload_state();
                            *STATE.show_log_selection.lock().unwrap() = false;
                            *STATE.show_token_input.lock().unwrap() = true;
                        });
                    }

                    ui.same_line();

                    if ui.button("No, Continue") {
                        ui.close_current_popup();
                    }
                });
        }
        ProcessingState::Idle => {
            let logs = STATE.logs.lock().unwrap();
            let selected_logs: Vec<_> = logs.iter().filter(|l| l.selected).collect();
            let total = selected_logs.len();
            let uploaded = selected_logs.iter().filter(|l| l.uploaded || l.status.starts_with("Failed")).count();
            drop(logs);
            
            if uploaded >= total && total > 0 {
                ui.text_colored([0.0, 1.0, 0.0, 1.0], "All files uploaded successfully!");
                ui.spacing();
                
                if ui.button("Start Processing") {
                    *STATE.processing_state.lock().unwrap() = ProcessingState::Processing;
                    
                    thread::spawn(|| {
                        let settings = Settings::get();
                        let api_endpoint = settings.api_endpoint.clone();
                        let history_token = settings.history_token.clone();
                        drop(settings);
                        
                        let session_id = STATE.session_id.lock().unwrap().clone();
                        let ownership_token = STATE.ownership_token.lock().unwrap().clone();
                        
                        log::info!("Starting processing");
                        match upload::start_processing(&api_endpoint, &session_id, &history_token, &ownership_token) {
                            Ok(server_message) => {
                                log::info!("Processing started successfully: {}", server_message);
                                *STATE.last_status_check.lock().unwrap() = Some(std::time::Instant::now());
                            }
                            Err(e) => {
                                log::error!("Failed to start processing: {}", e);
                                *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
                                *STATE.report_url.lock().unwrap() = format!("Server error: {}", e);
                            }
                        }
                    });
                }
                
                ui.same_line();
                
                if ui.button("Cancel") {
                    ui.open_popup("cancel_before_processing");
                }
                
                ui.popup_modal("cancel_before_processing")
                    .always_auto_resize(true)
                    .build(ui, || {
                        ui.text("Are you sure you want to cancel this job?");
                        ui.spacing();
                        ui.text_colored([1.0, 1.0, 0.0, 1.0], "The uploaded files will be abandoned.");
                        ui.spacing();

                        if ui.button("Yes, Cancel") {
                            ui.close_current_popup();
                            thread::spawn(|| {
                                log::info!("User cancelled before processing");
                                reset_upload_state();
                                *STATE.show_log_selection.lock().unwrap() = false;
                                *STATE.show_token_input.lock().unwrap() = true;
                            });
                        }

                        ui.same_line();

                        if ui.button("No, Continue") {
                            ui.close_current_popup();
                        }
                    });
            } else {
                ui.text("Uploading files...");
            }
        }
        ProcessingState::Processing => {
            let progress = *STATE.processing_progress.lock().unwrap();
            let phase = STATE.processing_phase.lock().unwrap().clone();
            
            if !phase.is_empty() {
                ui.text(&phase);
            } else {
                ui.text("Processing logs on server...");
            }
            
            ui.spacing();
            
            // Progress bar
            let progress_fraction = progress / 100.0;
            ui.text(format!("Progress: {:.0}%", progress));
            use nexus::imgui::ProgressBar;
            ProgressBar::new(progress_fraction)
                .size([0.0, 0.0])
                .build(ui);
            
            ui.spacing();
            ui.text_colored([1.0, 1.0, 0.0, 1.0], "This may take several minutes...");
            
            ui.spacing();
            ui.separator();
            ui.spacing();
            
            if ui.button("Cancel Processing") {
                ui.open_popup("cancel_processing");
            }
            
            ui.popup_modal("cancel_processing")
                .always_auto_resize(true)
                .build(ui, || {
                    ui.text("Are you sure you want to cancel this job?");
                    ui.spacing();
                    ui.text_colored([1.0, 1.0, 0.0, 1.0], "The server will finish processing in the background,");
                    ui.text_colored([1.0, 1.0, 0.0, 1.0], "but you won't be able to see the results.");
                    ui.spacing();

                    if ui.button("Yes, Cancel") {
                        ui.close_current_popup();
                        thread::spawn(|| {
                            log::info!("User cancelled processing");
                            reset_upload_state();
                            *STATE.show_log_selection.lock().unwrap() = false;
                            *STATE.show_token_input.lock().unwrap() = true;
                        });
                    }

                    ui.same_line();

                    if ui.button("No, Continue") {
                        ui.close_current_popup();
                    }
                });
        }
        ProcessingState::Complete => {
            ui.text_colored([0.0, 1.0, 0.0, 1.0], "Processing complete!");
        }
        ProcessingState::Failed => {
            let error_message = STATE.report_url.lock().unwrap().clone();
            drop(STATE.report_url.lock());
            
            ui.text_colored([1.0, 0.0, 0.0, 1.0], "Processing failed!");
            ui.spacing();
            
            if !error_message.is_empty() {
                ui.text("Server response:");
                ui.text_colored([1.0, 0.5, 0.5, 1.0], &error_message);
                ui.spacing();
            }
            
            if ui.button("Retry Processing") {
                *STATE.processing_state.lock().unwrap() = ProcessingState::Processing;
                STATE.report_url.lock().unwrap().clear();
                
                thread::spawn(|| {
                    let settings = Settings::get();
                    let api_endpoint = settings.api_endpoint.clone();
                    let history_token = settings.history_token.clone();
                    drop(settings);
                    
                    let session_id = STATE.session_id.lock().unwrap().clone();
                    let ownership_token = STATE.ownership_token.lock().unwrap().clone();
                    
                    log::info!("Retrying processing");
                    match upload::start_processing(&api_endpoint, &session_id, &history_token, &ownership_token) {
                        Ok(server_message) => {
                            log::info!("Processing started successfully: {}", server_message);
                            *STATE.last_status_check.lock().unwrap() = Some(std::time::Instant::now());
                        }
                        Err(e) => {
                            log::error!("Failed to start processing: {}", e);
                            *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
                            *STATE.report_url.lock().unwrap() = format!("Server error: {}", e);
                        }
                    }
                });
            }
            
            ui.same_line();
            
            if ui.button("Back to Log Selection") {
                thread::spawn(|| {
                    log::info!("Back to Log Selection clicked - spawning reset");
                    reset_upload_state();
                    log::info!("Reset complete");
                });
            }
        }
    }
}

fn render_results(ui: &Ui) {
    ui.text("Processing Complete!");
    ui.spacing();

    let report_url = STATE.report_url.lock().unwrap();
    ui.text("Your report is ready:");
    ui.text_wrapped(&*report_url);

    if ui.button("Copy URL") {
        ui.set_clipboard_text(&*report_url);
    }

    ui.same_line();

    if ui.button("Open in Browser") {
        if let Err(e) = open::that_detached(report_url.as_str()) {
            log::error!("Failed to open browser: {}", e);
        }
    }

    drop(report_url);

    ui.spacing();
    ui.separator();

    if ui.button("Upload More Logs") {
        log::info!("Upload More Logs button clicked");
        thread::spawn(|| {
            log::info!("Resetting upload state");
            reset_upload_state();
            log::info!("State reset complete, starting log scan");
            scan_for_logs();
            log::info!("Log scan complete");
        });
    }
    
    ui.same_line();
    
    if ui.button("Back to Start") {
        thread::spawn(|| {
            log::info!("Back to Start button clicked");
            reset_upload_state();
            *STATE.show_log_selection.lock().unwrap() = false;
            *STATE.show_token_input.lock().unwrap() = true;
        });
    }
}

fn reset_upload_state() {
    log::info!("reset_upload_state: Starting");
    
    log::info!("reset_upload_state: Resetting show_upload_progress");
    *STATE.show_upload_progress.lock().unwrap() = false;
    
    log::info!("reset_upload_state: Resetting show_results");
    *STATE.show_results.lock().unwrap() = false;
    
    log::info!("reset_upload_state: Resetting processing_state");
    *STATE.processing_state.lock().unwrap() = ProcessingState::Idle;
    
    log::info!("reset_upload_state: Clearing report_url");
    STATE.report_url.lock().unwrap().clear();
    
    log::info!("reset_upload_state: Clearing session_id");
    STATE.session_id.lock().unwrap().clear();
    
    log::info!("reset_upload_state: Clearing ownership_token");
    STATE.ownership_token.lock().unwrap().clear();
    
    log::info!("reset_upload_state: Resetting last_status_check");
    *STATE.last_status_check.lock().unwrap() = None;
    
    log::info!("reset_upload_state: Resetting processing_progress");
    *STATE.processing_progress.lock().unwrap() = 0.0;
    
    log::info!("reset_upload_state: Clearing processing_phase");
    STATE.processing_phase.lock().unwrap().clear();
    
    log::info!("reset_upload_state: Locking logs for reset");
    let mut logs = STATE.logs.lock().unwrap();
    log::info!("reset_upload_state: Got logs lock, resetting {} logs", logs.len());
    for log in logs.iter_mut() {
        log.selected = false;
        log.uploaded = false;
        log.status = "Ready".to_string();
    }
    drop(logs);
    log::info!("reset_upload_state: Logs reset complete");
    
    log::info!("reset_upload_state: Setting show_log_selection to true");
    *STATE.show_log_selection.lock().unwrap() = true;
    
    log::info!("reset_upload_state: Complete");
}

fn sync_with_arcdps() -> Result<String, String> {
    use std::os::windows::ffi::OsStringExt;
    use std::ffi::OsString;
    use winapi::um::libloaderapi::GetModuleFileNameW;
    use winapi::shared::minwindef::HMODULE;
    
    // Get GW2 executable path
    let mut buffer = [0u16; 4096];
    let len = unsafe {
        GetModuleFileNameW(std::ptr::null_mut() as HMODULE, buffer.as_mut_ptr(), buffer.len() as u32)
    };
    
    if len == 0 {
        return Err("Unable to locate Guild Wars 2 directory".to_string());
    }
    
    let exe_path = OsString::from_wide(&buffer[..len as usize]);
    let exe_path = PathBuf::from(exe_path);
    let gw2_dir = exe_path.parent().ok_or("Unable to determine GW2 directory")?;
    
    // Try multiple possible locations for arcdps.ini
    let possible_paths = [
        gw2_dir.join("arcdps.ini"),
        gw2_dir.join("addons").join("arcdps.ini"),
        gw2_dir.join("addons").join("arcdps").join("arcdps.ini"),
    ];
    
    for ini_path in &possible_paths {
        if ini_path.exists() {
            // Read the file
            if let Ok(contents) = std::fs::read_to_string(ini_path) {
                // Look for boss_encounter_path line
                for line in contents.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("boss_encounter_path=") {
                        let path = trimmed.trim_start_matches("boss_encounter_path=").trim();
                        if !path.is_empty() {
                            log::info!("Found ArcDPS log path: {}", path);
                            return Ok(path.to_string());
                        }
                    }
                }
            }
        }
    }
    
    Err("⚠ Unable to locate arcdps.ini or boss_encounter_path setting".to_string())
}

fn check_auto_cleanup_on_load() {
    let settings = Settings::get();
    let enabled = settings.auto_cleanup_enabled;
    let days = settings.auto_cleanup_days;
    let log_dir = settings.log_directory.clone();
    drop(settings);
    
    if !enabled {
        return;
    }
    
    // Check if already done this session
    let mut done = STATE.auto_cleanup_done.lock().unwrap();
    if *done {
        return;
    }
    *done = true;
    drop(done);
    
    log::info!("Auto-cleanup enabled, running cleanup for logs older than {} days", days);
    
    thread::spawn(move || {
        match cleanup_old_logs(&log_dir, days) {
            Ok((files, bytes)) => {
                let mb = bytes as f64 / 1024.0 / 1024.0;
                log::info!("Auto-cleanup complete: {} files ({:.2} MB) moved to Recycle Bin", files, mb);
            }
            Err(e) => {
                log::warn!("Auto-cleanup failed: {}", e);
            }
        }
    });
}

fn cleanup_old_logs(log_directory: &str, days_old: u32) -> Result<(usize, u64), String> {
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::shellapi::{SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT, FO_DELETE, SHFILEOPSTRUCTW};
    use winapi::shared::minwindef::TRUE;
    
    if log_directory.is_empty() {
        return Err("No log directory configured".to_string());
    }
    
    let log_dir = PathBuf::from(log_directory);
    
    if !log_dir.exists() {
        return Err("Log directory does not exist".to_string());
    }
    
    let log_dir = match log_dir.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            return Err(format!("Invalid directory path: {}", e));
        }
    };
    
    let log_dir_str = log_dir.to_string_lossy().to_lowercase();
    let is_root = log_dir_str.ends_with(":\\") || log_dir_str.ends_with(":/");
    
    if is_root || log_dir_str.contains("\\windows\\") || log_dir_str.contains("\\program files") {
        return Err("Cannot clean system directories or drive roots".to_string());
    }
    
    let cutoff_time = std::time::SystemTime::now() 
        - std::time::Duration::from_secs(days_old as u64 * 24 * 60 * 60);
    
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let temp_folder_name = format!("WvW_Insights_Cleanup_{}", timestamp);
    let temp_folder_path = log_dir.join(&temp_folder_name);
    
    if let Err(e) = std::fs::create_dir(&temp_folder_path) {
        return Err(format!("Failed to create temporary folder: {}", e));
    }
    
    let mut files_to_move = Vec::new();
    let mut total_size = 0u64;
    
    fn collect_old_logs_recursive(
        dir: &std::path::Path,
        cutoff: std::time::SystemTime,
        files: &mut Vec<PathBuf>,
        size: &mut u64,
        exclude_folder: &std::path::Path,
    ) -> Result<(), String> {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                return Err(format!("Failed to read directory: {}", e));
            }
        };
        
        for entry in entries.flatten() {
            let entry_path = entry.path();
            
            // Skip the temp folder we just created
            if entry_path == exclude_folder {
                continue;
            }
            
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            
            if metadata.is_dir() {
                // Skip ANY folder with "WvW_Insights_Cleanup" in its name
                if let Some(dir_name) = entry_path.file_name() {
                    if dir_name.to_string_lossy().contains("WvW_Insights_Cleanup") {
                        log::info!("Skipping cleanup temp folder: {:?}", entry_path);
                        continue;
                    }
                }
                collect_old_logs_recursive(&entry_path, cutoff, files, size, exclude_folder)?;
            } else if metadata.is_file() {
                if let Some(ext) = entry_path.extension() {
                    if ext == "zevtc" {
                        if let Ok(modified) = metadata.modified() {
                            if modified < cutoff {
                                files.push(entry_path);
                                *size += metadata.len();
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    collect_old_logs_recursive(&log_dir, cutoff_time, &mut files_to_move, &mut total_size, &temp_folder_path)
        .map_err(|e| format!("Failed to scan directory: {}", e))?;
    
    if files_to_move.is_empty() {
        let _ = std::fs::remove_dir(&temp_folder_path);
        return Ok((0, 0));
    }
    
    let files_count = files_to_move.len();
    
    let mut moved_count = 0;
    let mut moved_size = 0u64;
    
    for file in files_to_move.iter() {
        let file_name = match file.file_name() {
            Some(name) => name,
            None => continue,
        };
        
        let mut dest_path = temp_folder_path.join(file_name);
        let mut counter = 1;
        while dest_path.exists() {
            let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
            let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("zevtc");
            dest_path = temp_folder_path.join(format!("{}_{}.{}", stem, counter, ext));
            counter += 1;
        }
        
        let move_result = std::fs::rename(file, &dest_path)
            .or_else(|_| {
                std::fs::copy(file, &dest_path)
                    .and_then(|_| {
                        std::fs::remove_file(file)?;
                        Ok(())
                    })
            });
        
        if move_result.is_ok() {
            if let Ok(metadata) = std::fs::metadata(&dest_path) {
                moved_size += metadata.len();
            }
            moved_count += 1;
        }
    }
    
    if moved_count == 0 {
        let _ = std::fs::remove_dir(&temp_folder_path);
        return Err("Failed to move any files".to_string());
    }
    
    log::info!("Successfully moved {} files into temporary folder", moved_count);
    
    // Check if temp folder actually exists before attempting recycle
    if !temp_folder_path.exists() {
        log::error!("Temp folder doesn't exist after moving files: {:?}", temp_folder_path);
        return Err("Temp folder disappeared after moving files".to_string());
    }
    
    log::info!("Temp folder exists, attempting to send to Recycle Bin: {:?}", temp_folder_path);
    
    // CRITICAL FIX: Strip the \\?\ prefix that canonicalize adds
    // SHFileOperationW doesn't support the \\?\ prefix
    let path_for_shell = temp_folder_path.to_string_lossy();
    let path_for_shell = if path_for_shell.starts_with(r"\\?\") {
        &path_for_shell[4..]  // Remove \\?\ prefix
    } else {
        &path_for_shell
    };
    
    log::info!("Path for shell operation (without \\\\?\\ prefix): {}", path_for_shell);
    
    // Convert to wide string with double null terminator
    let mut path_buffer: Vec<u16> = std::ffi::OsStr::new(path_for_shell)
        .encode_wide()
        .chain(std::iter::once(0))
        .chain(std::iter::once(0))
        .collect();
    
    log::info!("Path buffer length: {}, last 4 values: {:?}", 
        path_buffer.len(), 
        &path_buffer[path_buffer.len().saturating_sub(4)..]);
    
    let mut file_op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE as u32,
        pFrom: path_buffer.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    
    log::info!("Calling SHFileOperationW...");
    let result = unsafe { SHFileOperationW(&mut file_op) };
    log::info!("SHFileOperationW returned: {}, aborted: {}", result, file_op.fAnyOperationsAborted);
    
    // Check if folder still exists after the operation
    let folder_still_exists = temp_folder_path.exists();
    log::info!("Temp folder exists after operation: {}", folder_still_exists);
    
    if result == 0 && file_op.fAnyOperationsAborted != TRUE {
        log::info!("Cleanup: {} files ({:.2} MB) moved to Recycle Bin", 
            moved_count, moved_size as f64 / 1024.0 / 1024.0);
        Ok((moved_count, moved_size))
    } else {
        log::error!("SHFileOperationW failed with code: {}, aborted: {}", result, file_op.fAnyOperationsAborted);
        
        // DON'T delete the folder - it contains user's files!
        if folder_still_exists {
            log::warn!("Temp folder still exists at: {:?}", temp_folder_path);
            log::warn!("User can manually move this folder to Recycle Bin");
        } else {
            log::error!("WARNING: Temp folder disappeared but wasn't sent to Recycle Bin!");
        }
        
        Err(format!("Failed to move folder to Recycle Bin (error: {}, folder exists: {})", result, folder_still_exists))
    }
}

fn render_settings(ui: &Ui) {
    thread_local! {
        static LOG_DIR_BUFFER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static API_ENDPOINT_BUFFER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static SHOW_FORMATTED: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
        static INITIALIZED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static NEW_TOKEN_NAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static NEW_TOKEN_VALUE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        static TOKEN_TO_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
        static REPORT_TO_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
        static ACTIVE_TAB: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static CLEANUP_DAYS: std::cell::Cell<i32> = const { std::cell::Cell::new(30) };
    }

    if !INITIALIZED.get() {
        let settings = Settings::get();
        LOG_DIR_BUFFER.set(settings.log_directory.clone());
        API_ENDPOINT_BUFFER.set(settings.api_endpoint.clone());
        SHOW_FORMATTED.set(settings.show_formatted_timestamps);
        INITIALIZED.set(true);
    }

    ui.text("Settings");
    ui.separator();
    ui.spacing();

    // Tab buttons
    let mut active_tab = ACTIVE_TAB.get();
    
    if ui.button("General") {
        active_tab = 0;
        ACTIVE_TAB.set(0);
    }
    ui.same_line();
    if ui.button("Token Manager") {
        active_tab = 1;
        ACTIVE_TAB.set(1);
    }
    ui.same_line();
    if ui.button("Report History") {
        active_tab = 2;
        ACTIVE_TAB.set(2);
    }
    ui.same_line();
    if ui.button("Cleanup") {
        active_tab = 3;
        ACTIVE_TAB.set(3);
    }
    
    ui.spacing();
    ui.separator();
    ui.spacing();

    // Tab content
    match active_tab {
        0 => {
            // General Settings Tab
            
            // Check if sync operation completed
            let sync_result = STATE.sync_arcdps_result.lock().unwrap().take();
            if let Some(result) = sync_result {
                match result {
                    Ok(path) => {
                        LOG_DIR_BUFFER.set(path);
                        *STATE.sync_arcdps_message.lock().unwrap() = "✓ Synced successfully!".to_string();
                        *STATE.sync_arcdps_message_is_error.lock().unwrap() = false;
                        *STATE.sync_arcdps_message_until.lock().unwrap() = 
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                    }
                    Err(e) => {
                        *STATE.sync_arcdps_message.lock().unwrap() = format!("⚠ {}", e);
                        *STATE.sync_arcdps_message_is_error.lock().unwrap() = true;
                        *STATE.sync_arcdps_message_until.lock().unwrap() = 
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                    }
                }
            }
            
            ui.text("Log Directory:");
            
            // Log directory input
            LOG_DIR_BUFFER.with_borrow_mut(|dir| {
                ui.input_text("##logdir", dir)
                    .hint("e.g., D:\\LOGS\\arcdps.cbtlogs\\1")
                    .build();
            });
            
            // Sync with ArcDPS button (same line)
            ui.same_line();
            
            let is_syncing = *STATE.sync_arcdps_pending.lock().unwrap();
            if is_syncing {
                let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
                let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
                let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
                ui.button("Syncing...");
            } else {
                if ui.button("Sync with ArcDPS") {
                    *STATE.sync_arcdps_pending.lock().unwrap() = true;
                    thread::spawn(|| {
                        let result = sync_with_arcdps();
                        *STATE.sync_arcdps_result.lock().unwrap() = Some(result);
                        *STATE.sync_arcdps_pending.lock().unwrap() = false;
                    });
                }
            }
            
            // Show temporary message next to button
            let message_until = *STATE.sync_arcdps_message_until.lock().unwrap();
            if let Some(until) = message_until {
                if std::time::Instant::now() < until {
                    ui.same_line();
                    let message = STATE.sync_arcdps_message.lock().unwrap().clone();
                    let is_error = *STATE.sync_arcdps_message_is_error.lock().unwrap();
                    
                    let color = if is_error {
                        [1.0, 0.5, 0.0, 1.0] // Orange for errors
                    } else {
                        [0.0, 1.0, 0.0, 1.0] // Green for success
                    };
                    
                    ui.text_colored(color, &message);
                } else {
                    // Message expired, clear it
                    *STATE.sync_arcdps_message_until.lock().unwrap() = None;
                }
            }
            
            ui.text_colored([0.7, 0.7, 0.7, 1.0], "The folder containing your .zevtc log files");
            ui.text_colored([0.7, 0.7, 0.7, 1.0], "Subdirectories will be scanned recursively");
            
            ui.spacing();
            ui.separator();
            ui.spacing();

            ui.text("Display Options:");
            let mut show_formatted = SHOW_FORMATTED.get();
            if ui.checkbox("Show formatted timestamps", &mut show_formatted) {
                SHOW_FORMATTED.set(show_formatted);
            }
            ui.text_colored([0.7, 0.7, 0.7, 1.0], "Display readable dates instead of raw filenames");
            
            ui.spacing();
            ui.separator();
            ui.spacing();

            ui.text("API Endpoint:");
            API_ENDPOINT_BUFFER.with_borrow_mut(|endpoint| {
                ui.input_text("##apiendpoint", endpoint).build();
            });
            
            if ui.button("Reset to Default") {
                API_ENDPOINT_BUFFER.set("https://parser.rethl.net/api.php".to_string());
            }
            
            ui.text_colored([0.7, 0.7, 0.7, 1.0], "Leave as default unless instructed otherwise");
        }
        1 => {
            // Token Manager Tab
            ui.text("Saved Tokens:");
            ui.spacing();

            let mut settings = Settings::get();
            let saved_tokens = settings.saved_tokens.clone();
            drop(settings);

            if saved_tokens.is_empty() {
                ui.text_colored([0.7, 0.7, 0.7, 1.0], "No saved tokens yet");
            } else {
                ChildWindow::new("SavedTokensList")
                    .size([0.0, 200.0])
                    .build(ui, || {
                        for (index, saved_token) in saved_tokens.iter().enumerate() {
                            ui.text(&saved_token.name);
                            ui.same_line();
                            
                            let masked = if saved_token.token.len() > 8 {
                                format!("{}...{}", 
                                    &saved_token.token[..4], 
                                    &saved_token.token[saved_token.token.len()-4..])
                            } else {
                                "****".to_string()
                            };
                            ui.text_colored([0.5, 0.5, 0.5, 1.0], &masked);
                            
                            ui.same_line();
                            
                            if ui.small_button(&format!("Use##use_{}", index)) {
                                let mut settings = Settings::get();
                                settings.history_token = saved_token.token.clone();
                                if let Err(e) = settings.store(config_path()) {
                                    log::error!("Failed to save settings: {}", e);
                                }
                                log::info!("Switched to token: {}", saved_token.name);
                            }
                            
                            ui.same_line();
                            
                            if ui.small_button(&format!("Delete##del_{}", index)) {
                                TOKEN_TO_DELETE.set(Some(index));
                            }
                            
                            ui.spacing();
                        }
                    });
            }

            if let Some(index_to_delete) = TOKEN_TO_DELETE.get() {
                let mut settings = Settings::get();
                if index_to_delete < settings.saved_tokens.len() {
                    let deleted_name = settings.saved_tokens[index_to_delete].name.clone();
                    settings.saved_tokens.remove(index_to_delete);
                    if let Err(e) = settings.store(config_path()) {
                        log::error!("Failed to save settings after deletion: {}", e);
                    } else {
                        log::info!("Deleted token: {}", deleted_name);
                    }
                }
                TOKEN_TO_DELETE.set(None);
            }

            ui.spacing();
            ui.separator();
            ui.spacing();

            ui.text("Save New Token:");
            ui.spacing();

            ui.text_colored([0.9, 0.9, 0.9, 1.0], "Token Name:");
            NEW_TOKEN_NAME.with_borrow_mut(|name| {
                ui.input_text("##newTokenName", name).build();
            });
            ui.text_colored([0.6, 0.6, 0.6, 1.0], "(e.g., Main Account, Alt Account)");

            ui.spacing();

            ui.text_colored([0.9, 0.9, 0.9, 1.0], "Token Value:");
            NEW_TOKEN_VALUE.with_borrow_mut(|token| {
                ui.input_text("##newTokenValue", token).build();
            });
            ui.text_colored([0.6, 0.6, 0.6, 1.0], "(Paste your history token here)");

            ui.spacing();

            let can_save = NEW_TOKEN_NAME.with_borrow(|name| !name.trim().is_empty()) 
                && NEW_TOKEN_VALUE.with_borrow(|token| !token.trim().is_empty());

            if can_save {
                if ui.button("Save Token") {
                    NEW_TOKEN_NAME.with_borrow(|name| {
                        NEW_TOKEN_VALUE.with_borrow(|token| {
                            let mut settings = Settings::get();
                            settings.saved_tokens.push(SavedToken {
                                name: name.trim().to_string(),
                                token: token.trim().to_string(),
                            });
                            if let Err(e) = settings.store(config_path()) {
                                log::error!("Failed to save token: {}", e);
                            } else {
                                log::info!("Saved new token: {}", name.trim());
                            }
                        });
                    });
                    
                    NEW_TOKEN_NAME.set(String::new());
                    NEW_TOKEN_VALUE.set(String::new());
                }
            } else {
                let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
                let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
                let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
                ui.button("Save Token");
            }
        }
        2 => {
            // Report History Tab
            ui.text("Your Report History:");
            ui.spacing();

            let mut settings = Settings::get();
            let current_token = settings.history_token.clone();
            let mut report_history = settings.report_history.clone();
            drop(settings);

            // Sort by timestamp (newest first)
            report_history.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

            if report_history.is_empty() {
                ui.text_colored([0.7, 0.7, 0.7, 1.0], "No reports yet");
                ui.spacing();
                ui.text_colored([0.7, 0.7, 0.7, 1.0], "Complete a parse to see it here!");
            } else {
                ui.text_colored([0.7, 0.7, 0.7, 1.0], &format!("Total reports: {}", report_history.len()));
                ui.spacing();

                if ui.button("Clear All History") {
                    ui.open_popup("clear_history_confirmation");
                }

                ui.popup_modal("clear_history_confirmation")
                    .always_auto_resize(true)
                    .build(ui, || {
                        ui.text("Are you sure you want to clear all report history?");
                        ui.spacing();
                        ui.text_colored([1.0, 1.0, 0.0, 1.0], "This cannot be undone!");
                        ui.spacing();

                        if ui.button("Yes, Clear All") {
                            ui.close_current_popup();
                            let mut settings = Settings::get();
                            settings.report_history.clear();
                            if let Err(e) = settings.store(config_path()) {
                                log::error!("Failed to save settings: {}", e);
                            }
                            log::info!("Cleared all report history");
                        }

                        ui.same_line();

                        if ui.button("Cancel") {
                            ui.close_current_popup();
                        }
                    });

                ui.spacing();
                ui.separator();
                ui.spacing();

                ChildWindow::new("ReportHistoryList")
                    .size([0.0, 350.0])
                    .build(ui, || {
                        for (index, entry) in report_history.iter().enumerate() {
                            let timestamp_str = format_report_timestamp(entry.timestamp);
                            
                            ui.text_colored([0.8, 0.8, 1.0, 1.0], &timestamp_str);
                            ui.text_colored([0.6, 0.6, 0.6, 1.0], &format!("Session: {}", entry.session_id));
                            
                            if ui.small_button(&format!("Copy URL##copy_{}", index)) {
                                ui.set_clipboard_text(&entry.url);
                                log::info!("Copied URL to clipboard");
                            }
                            
                            ui.same_line();
                            
                            if ui.small_button(&format!("Open in Browser##open_{}", index)) {
                                if let Err(e) = open::that_detached(&entry.url) {
                                    log::error!("Failed to open browser: {}", e);
                                }
                            }
                            
                            ui.same_line();
                            
                            if ui.small_button(&format!("Delete##del_{}", index)) {
                                REPORT_TO_DELETE.set(Some(index));
                            }
                            
                            ui.spacing();
                            ui.separator();
                            ui.spacing();
                        }
                    });
            }

            // Handle deletion
            if let Some(index_to_delete) = REPORT_TO_DELETE.get() {
                let mut settings = Settings::get();
                // Sort the same way to match indices
                settings.report_history.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                if index_to_delete < settings.report_history.len() {
                    settings.report_history.remove(index_to_delete);
                    if let Err(e) = settings.store(config_path()) {
                        log::error!("Failed to save settings after deletion: {}", e);
                    } else {
                        log::info!("Deleted report from history");
                    }
                }
                REPORT_TO_DELETE.set(None);
            }

            ui.spacing();
            ui.separator();
            ui.spacing();

            // View all reports on website button
            ui.text_colored([0.7, 0.7, 0.7, 1.0], "View all reports parsed with your current token:");
            ui.spacing();

            if !current_token.is_empty() {
                if ui.button("View All Reports on Website") {
                    let url = format!("https://parser.rethl.net/?hisToken={}", current_token);
                    if let Err(e) = open::that_detached(&url) {
                        log::error!("Failed to open browser: {}", e);
                    } else {
                        log::info!("Opening all reports on website with token");
                    }
                }
                
                ui.same_line();
                
                if ui.small_button("Copy Link") {
                    let url = format!("https://parser.rethl.net/?hisToken={}", current_token);
                    ui.set_clipboard_text(&url);
                    log::info!("Copied website URL to clipboard");
                }
            } else {
                let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
                let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
                let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
                ui.button("View All Reports on Website");
                drop(_style3);
                drop(_style2);
                drop(_style);
                
                if ui.is_item_hovered() {
                    ui.tooltip_text("Enter a history token first");
                }
            }
        }
        3 => {
            // Cleanup Tab
            
            let cleanup_result = STATE.cleanup_result.lock().unwrap().take();
            if let Some(result) = cleanup_result {
                match result {
                    Ok((files, bytes)) => {
                        let mb = bytes as f64 / 1024.0 / 1024.0;
                        *STATE.cleanup_message_until.lock().unwrap() = 
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                    }
                    Err(_) => {
                        *STATE.cleanup_message_until.lock().unwrap() = 
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                    }
                }
            }
            
            ui.text_colored([1.0, 0.8, 0.2, 1.0], "Log Cleanup");
            ui.spacing();
            ui.text_wrapped("Move old log files to Recycle Bin to free up disk space.");
            ui.spacing();
            ui.separator();
            ui.spacing();
            
            // Auto-cleanup section
            let mut settings = Settings::get();
            let mut auto_enabled = settings.auto_cleanup_enabled;
            let mut auto_days = settings.auto_cleanup_days as i32;
            drop(settings);
            
            ui.text_colored([1.0, 1.0, 0.0, 1.0], "âš ï¸ Automatic Cleanup");
            ui.spacing();
            
            if ui.checkbox("Enable automatic cleanup on plugin load", &mut auto_enabled) {
                if auto_enabled {
                    // Show warning when enabling
                    ui.open_popup("auto_cleanup_warning");
                } else {
                    // Save immediately when disabling
                    let mut settings = Settings::get();
                    settings.auto_cleanup_enabled = false;
                    if let Err(e) = settings.store(config_path()) {
                        log::error!("Failed to save settings: {}", e);
                    }
                }
            }
            
        // Warning popup when enabling auto-cleanup
        ui.popup_modal("auto_cleanup_warning")
            .always_auto_resize(true)
            .build(ui, || {
                ui.text_colored([1.0, 0.0, 0.0, 1.0], "âš ï¸ WARNING âš ï¸");
                ui.spacing();
                ui.text_wrapped("Automatic cleanup will run ONCE when the plugin loads");
                ui.text_wrapped("(each time you start Guild Wars 2).");
                ui.spacing();
                ui.text_wrapped("Old logs will be moved to the Recycle Bin automatically");
                ui.text_wrapped("without confirmation.");
                ui.spacing();
                ui.separator();
                ui.spacing();
                
                if ui.button("Enable Automatic Cleanup") {
                    ui.close_current_popup();
                    let mut settings = Settings::get();
                    settings.auto_cleanup_enabled = true;
                    if let Err(e) = settings.store(config_path()) {
                        log::error!("Failed to save settings: {}", e);
                    }
                }
                
                ui.same_line();
                
                if ui.button("Cancel") {
                    ui.close_current_popup();
                }
            });
            
        if auto_enabled {
            ui.text_colored([0.7, 0.7, 0.7, 1.0], "Auto-cleanup will run once per session");
            ui.spacing();
            
            ui.text("Delete logs older than:");
            ui.set_next_item_width(100.0);
            if ui.input_int("##auto_cleanup_days", &mut auto_days).build() {
                auto_days = auto_days.max(1).min(9999);
                let mut settings = Settings::get();
                settings.auto_cleanup_days = auto_days as u32;
                if let Err(e) = settings.store(config_path()) {
                    log::error!("Failed to save settings: {}", e);
                }
            }
            ui.same_line();
            ui.text("days");
        }
            
            ui.spacing();
            ui.separator();
            ui.spacing();
            
            // Manual cleanup section
            ui.text_colored([1.0, 1.0, 1.0, 1.0], "Manual Cleanup");
            ui.spacing();
            ui.text("Delete logs older than:");
            ui.spacing();
            
            let mut days = CLEANUP_DAYS.get();
            ui.set_next_item_width(100.0);
            if ui.input_int("##cleanup_days", &mut days).build() {
                days = days.max(1).min(9999);
                CLEANUP_DAYS.set(days);
            }
            ui.same_line();
            ui.text("days");
            
            ui.spacing();
            ui.separator();
            ui.spacing();
            
            let settings = Settings::get();
            let log_dir = settings.log_directory.clone();
            drop(settings);
            
            if log_dir.is_empty() {
                ui.text_colored([1.0, 0.0, 0.0, 1.0], "âš  No log directory configured!");
                ui.spacing();
                ui.text_wrapped("Please set a log directory in the General tab first.");
            } else {
                ui.text_colored([0.7, 0.7, 0.7, 1.0], "Target directory:");
                ui.text_wrapped(&log_dir);
                ui.spacing();
                
                ui.spacing();
                ui.text_colored([1.0, 0.8, 0.0, 1.0], "âš  WARNING: Files will be moved to Recycle Bin");
                ui.text_colored([0.7, 0.7, 0.7, 1.0], "You can restore them from the Recycle Bin if needed");
                ui.spacing();
                
                let is_cleaning = *STATE.cleanup_in_progress.lock().unwrap();
                
                if is_cleaning {
                    let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
                    let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
                    let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
                    ui.button("Cleaning...");
                } else {
                    let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.8, 0.2, 0.2, 1.0]);
                    let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [1.0, 0.3, 0.3, 1.0]);
                    let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.6, 0.1, 0.1, 1.0]);
                    
                    if ui.button("Delete Old Logs") {
                        ui.open_popup("confirm_cleanup");
                    }
                }
                
            ui.popup_modal("confirm_cleanup")
                .always_auto_resize(true)
                .build(ui, || {
                    ui.text_colored([1.0, 0.0, 0.0, 1.0], "âš  FINAL WARNING âš ");
                    ui.spacing();
                    ui.text_wrapped(&format!(
                        "You are about to move all .zevtc files older than {} days to the Recycle Bin from:",
                        days
                    ));
                    ui.spacing();
                    ui.text_colored([1.0, 1.0, 0.0, 1.0], &log_dir);
                    ui.spacing();
                    ui.text_colored([1.0, 1.0, 0.0, 1.0], "Files can be restored from the Recycle Bin if needed.");
                    ui.spacing();
                    ui.separator();
                    ui.spacing();
                    
                    let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.8, 0.2, 0.2, 1.0]);
                    let _style2 = ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [1.0, 0.3, 0.3, 1.0]);
                    let _style3 = ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.6, 0.1, 0.1, 1.0]);
                    
                    if ui.button("Yes, Move to Recycle Bin") {
                        ui.close_current_popup();
                        *STATE.cleanup_in_progress.lock().unwrap() = true;
                        
                        let days_to_delete = days as u32;
                        let dir_to_clean = log_dir.clone();
                        
                        thread::spawn(move || {
                            let result = cleanup_old_logs(&dir_to_clean, days_to_delete);
                            *STATE.cleanup_result.lock().unwrap() = Some(result);
                            *STATE.cleanup_in_progress.lock().unwrap() = false;
                        });
                    }
                    
                    ui.same_line();
                    
                    if ui.button("Cancel") {
                        ui.close_current_popup();
                    }
                });
                
                let last_result = STATE.cleanup_result.lock().unwrap();
                let message_until = *STATE.cleanup_message_until.lock().unwrap();
                
                if let Some(until) = message_until {
                    if std::time::Instant::now() < until {
                        ui.spacing();
                        ui.separator();
                        ui.spacing();
                        
                        if let Some(ref result) = *last_result {
                            match result {
                                Ok((files, bytes)) => {
                                    let mb = *bytes as f64 / 1024.0 / 1024.0;
                                    ui.text_colored([0.0, 1.0, 0.0, 1.0], 
                                        &format!("✓ Cleanup complete: {} files deleted, {:.2} MB freed", files, mb));
                                }
                                Err(e) => {
                                    ui.text_colored([1.0, 0.0, 0.0, 1.0], &format!("âœ— {}", e));
                                }
                            }
                        }
                    } else {
                        drop(last_result);
                        *STATE.cleanup_message_until.lock().unwrap() = None;
                    }
                } else {
                    drop(last_result);
                }
            }
        }
        _ => {}
    }

    ui.spacing();
    ui.separator();
    ui.spacing();

    if ui.button("Save & Return") {
        LOG_DIR_BUFFER.with_borrow(|dir| {
            API_ENDPOINT_BUFFER.with_borrow(|endpoint| {
                let mut settings = Settings::get();
                settings.log_directory = dir.clone();
                settings.api_endpoint = endpoint.clone();
                settings.show_formatted_timestamps = SHOW_FORMATTED.get();
                
                if let Err(e) = settings.store(config_path()) {
                    log::error!("Failed to save settings: {}", e);
                }
            });
        });

        *STATE.show_settings.lock().unwrap() = false;
        *STATE.show_token_input.lock().unwrap() = true;
        INITIALIZED.set(false);
    }
}

fn render_fn(ui: &Ui) {
    update_logs();
    check_upload_progress();
    check_auto_scan();  
    update_scan_display();  // NEW LINE
    
    // Only show window if show_main_window is true
    let show_window = *STATE.show_main_window.lock().unwrap();
    if !show_window {
        return;
    }
    
    let mut is_open = true;
    
    if let Some(_w) = Window::new("WvW Insights")
        .size([500.0, 600.0], nexus::imgui::Condition::FirstUseEver)
        .opened(&mut is_open)
        .begin(ui)
    {
        let show_token = *STATE.show_token_input.lock().unwrap();
        let show_logs = *STATE.show_log_selection.lock().unwrap();
        let show_progress = *STATE.show_upload_progress.lock().unwrap();
        let show_results = *STATE.show_results.lock().unwrap();
        let show_settings = *STATE.show_settings.lock().unwrap();

        if show_settings {
            render_settings(ui);
        } else if show_token {
            render_token_input(ui);
        } else if show_logs {
            render_log_selection(ui);
        } else if show_progress {
            render_upload_progress(ui);
        } else if show_results {
            render_results(ui);
        }
    }
    
    // Update window visibility if user closed it
    if !is_open {
        *STATE.show_main_window.lock().unwrap() = false;
        log::info!("Window closed by user");
    }
}

fn load() {
    log::info!("WvW Insights: Starting load");
    
    // Capture the addon load time
    *STATE.addon_load_time.lock().unwrap() = Some(std::time::Instant::now());
    
    let cfg_path = config_path();
    Settings::from_path(&cfg_path).unwrap_or_else(|e| {
        log::error!("Failed to load settings: {e}");
        Settings::get().init();
    });

    check_auto_cleanup_on_load();

    let producer_tx = STATE.init_producer();
    let upload_rx = STATE.init_upload_worker();
    
    let handle = upload::run(upload_rx, producer_tx);
    STATE.append_thread(handle);

    register_render(RenderType::Render, render!(render_fn)).revert_on_unload();
    
    // Load textures from embedded resources
    log::info!("Loading embedded icon textures");
    load_texture_from_memory(
        "ICON_WVW_INSIGHTS",
        ICON_NORMAL,
        Some(texture_receive!(handle_texture_receive)),
    );
    
    load_texture_from_memory(
        "ICON_WVW_INSIGHTS_HOVER",
        ICON_HOVER,
        Some(texture_receive!(handle_texture_receive)),
    );
    
    // Register keybind for toggling window
    register_keybind_with_string(
        "KB_WVW_INSIGHTS_TOGGLE",
        keybind_handler!(handle_toggle_keybind),
        "CTRL+SHIFT+W",
    )
    .revert_on_unload();
    
    // Add context menu shortcut (right-click menu on Nexus icon)
    add_quick_access_context_menu(
        "QAS_WVW_INSIGHTS",
        None::<&str>,  // target_identifier: None means it appears in the main Nexus right-click menu
        render!(render_simple_shortcut),
    )
    .revert_on_unload();
    
    // Add icon shortcut (will show up next to Nexus icon)
    add_quick_access(
        "QA_WVW_INSIGHTS",
        "ICON_WVW_INSIGHTS",
        "ICON_WVW_INSIGHTS_HOVER",
        "KB_WVW_INSIGHTS_TOGGLE",
        "Open WvW Insights - Upload and analyze your WvW combat logs",
    )
    .revert_on_unload();
    
    log::info!("WvW Insights: Load complete");
}

fn unload() {
    log::info!("WvW Insights: Starting unload");
    
    let settings = Settings::get();
    if let Err(e) = settings.store(config_path()) {
        log::error!("Failed to store settings: {e}");
    }
    drop(settings);

    drop(STATE.producer_rx.lock().unwrap().take());
    drop(STATE.upload_worker.lock().unwrap().take());

    for t in STATE.threads.lock().unwrap().drain(..) {
        let threadname = t
            .thread()
            .name()
            .map(String::from)
            .unwrap_or_else(|| format!("{:?}", t.thread().id()));
        log::trace!("Waiting on thread {}", threadname);
        if let Err(e) = t.join() {
            log::error!("Failed to join thread {}: {:#?}", threadname, e);
        }
    }

    log::info!("WvW Insights: Unload complete");
}

nexus::export! {
    name: "WvW Insights",
    signature: -12345,
    flags: AddonFlags::None,
    load,
    unload,
    provider: UpdateProvider::GitHub,
    update_link: "https://github.com/Retherichus/wvw-insights",
    log_filter: "warn,wvw_insights=info"
}
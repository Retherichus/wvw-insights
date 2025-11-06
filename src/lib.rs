use std::path::PathBuf;

use nexus::{
    gui::{register_render, RenderType},
    imgui::{Ui, Window},
    keybind::{keybind_handler, register_keybind_with_string},
    paths::get_addon_dir,
    quick_access::{add_quick_access, add_quick_access_context_menu},
    render, texture_receive,
    texture::{load_texture_from_memory, Texture},
    AddonFlags, UpdateProvider,
};

mod arcdps;
mod cleanup;
mod common;
mod formatting;
mod logfile;
mod scanning;
mod settings;
mod state;
mod qol;
mod tokens;
mod ui;
mod upload;
mod uploaded_logs;
use uploaded_logs::UploadedLogs;
mod webhooks;
use webhooks::WebhookSettings;
mod report_history;
use report_history::ReportHistory;

use cleanup::check_auto_cleanup_on_load;
use common::{WorkerMessage, WorkerType};
use scanning::{check_auto_scan, update_scan_display};
use settings::Settings;
use state::{ProcessingState, STATE};
mod upload_review;

// Embed icon resources at compile time
const ICON_NORMAL: &[u8] = include_bytes!("Icon.png");
const ICON_HOVER: &[u8] = include_bytes!("Icon_Hover.png");

fn config_path() -> PathBuf {
    get_addon_dir("wvw-insights")
        .expect("Addon dir to exist")
        .join("settings.json")
}

fn uploaded_logs_path() -> PathBuf {
    get_addon_dir("wvw-insights")
        .expect("Addon dir to exist")
        .join("uploaded_logs.json")
}

fn webhooks_path() -> PathBuf {
    get_addon_dir("wvw-insights")
        .expect("Addon dir to exist")
        .join("webhooks.json")
}

fn report_history_path() -> PathBuf {
    get_addon_dir("wvw-insights")
        .expect("Addon dir to exist")
        .join("report_history.json")
}

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
            *STATE.icon_texture.lock().unwrap() =
                texture.map(|t| unsafe { &*(t as *const Texture) });
            log::info!("Loaded WvW Insights icon texture");
        }
        "ICON_WVW_INSIGHTS_HOVER" => {
            *STATE.icon_hover_texture.lock().unwrap() =
                texture.map(|t| unsafe { &*(t as *const Texture) });
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

/// Updates the log list with results from upload workers
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

/// Checks the upload and processing progress
fn check_upload_progress() {
    let state = *STATE.processing_state.lock().unwrap();

    if state == ProcessingState::Uploading {
        let logs = STATE.logs.lock().unwrap();
        let selected_logs: Vec<_> = logs.iter().filter(|l| l.selected).collect();
        let total = selected_logs.len();
        let uploaded = selected_logs
            .iter()
            .filter(|l| l.uploaded || l.status.starts_with("Failed"))
            .count();
        drop(logs);

        if uploaded >= total && total > 0 {
            log::info!("All uploads complete ({}/{}), showing review screen", uploaded, total);
            
            // Transition to review screen instead of idle
            *STATE.processing_state.lock().unwrap() = ProcessingState::Idle;
            *STATE.show_upload_progress.lock().unwrap() = false;
            *STATE.show_upload_review.lock().unwrap() = true;
        }
    } else if state == ProcessingState::Processing {
        // Poll for completion every 3 seconds
        let mut last_check = STATE.last_status_check.lock().unwrap();
        let should_check = last_check
            .as_ref()
            .map_or(true, |t| t.elapsed() >= std::time::Duration::from_secs(3));
        if should_check {
            *last_check = Some(std::time::Instant::now());
            drop(last_check);

            std::thread::spawn(|| {
                let settings = Settings::get();
                let api_endpoint = settings.api_endpoint.clone();
                drop(settings);

                let session_id = STATE.session_id.lock().unwrap().clone();

                match upload::check_status(&api_endpoint, &session_id) {
                    Ok((status, report_urls, progress, phase)) => {
                        // Update progress and phase
                        *STATE.processing_progress.lock().unwrap() = progress;
                        if let Some(phase_msg) = phase {
                            *STATE.processing_phase.lock().unwrap() = phase_msg;
                        }
                        if status == "complete" {
                            log::info!("Processing complete!");
                            if let Some(urls) = report_urls {
                                *STATE.report_urls.lock().unwrap() = urls.clone();

                                // Save to new report history system
                                let session_id = STATE.session_id.lock().unwrap().clone();
                                let timestamp = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();

                                let mut history = ReportHistory::get();
                                
                                // First URL is always the main report
                                let main_url = urls[0].clone();
                                // Second URL (if exists) is the legacy report
                                let legacy_url = urls.get(1).cloned();
                                
                                history.add_report(session_id, timestamp, main_url, legacy_url);
                                
                                if let Err(e) = history.store(report_history_path()) {
                                    log::error!("Failed to save report history: {}", e);
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

/// Main render function
fn render_fn(ui: &Ui) {
    update_logs();
    check_upload_progress();
    check_auto_scan();
    update_scan_display();
    qol::update_mouse_lock();

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
        if ui.is_window_focused() && ui.is_key_pressed(nexus::imgui::Key::Escape) {
            *STATE.show_main_window.lock().unwrap() = false;
            log::info!("Window closed with ESC key");
            is_open = false;
        }

        let show_token = *STATE.show_token_input.lock().unwrap();
        let show_logs = *STATE.show_log_selection.lock().unwrap();
        let show_progress = *STATE.show_upload_progress.lock().unwrap();
        let show_review = *STATE.show_upload_review.lock().unwrap();
        let show_results = *STATE.show_results.lock().unwrap();
        let show_settings = *STATE.show_settings.lock().unwrap();

        let cfg_path = config_path();

        if show_settings {
            ui::render_settings(ui, &cfg_path);
        } else if show_token {
            ui::render_token_input(ui, &cfg_path);
        } else if show_logs {
            ui::render_log_selection(ui);
        } else if show_progress {
            ui::render_upload_progress(ui);
        } else if show_review {
            upload_review::render_upload_review(ui);
        } else if show_results {
            ui::render_results(ui);
        }
    }
    
    if !is_open {
        *STATE.show_main_window.lock().unwrap() = false;
        log::info!("Window closed by user");
    }
}

fn load() {
    log::info!("WvW Insights: Starting load");

    // Capture the addon load time
    *STATE.addon_load_time.lock().unwrap() = Some(std::time::Instant::now());
    
    qol::init_window_handle();

    let cfg_path = config_path();
    if let Err(e) = Settings::from_path(&cfg_path) {
        log::error!("Failed to load settings: {e}");
        let mut settings = Settings::get();
        settings.init();
        if let Err(e) = settings.store(&cfg_path) {
            log::error!("Failed to save initialized settings: {e}");
        }
        log::info!("Settings initialized with defaults and saved");
    }
    log::info!("Settings loaded - log_directory: {}", Settings::get().log_directory);

    // Load uploaded logs history
    let uploaded_path = uploaded_logs_path();
    if let Err(e) = UploadedLogs::from_path(&uploaded_path) {
        log::warn!("Failed to load uploaded logs history: {e}");
    }
    
    // Clean up uploaded logs older than 72 hours
    {
        let mut uploaded = UploadedLogs::get();
        let removed = uploaded.cleanup_old_entries();
        
        // Save after cleanup if anything was removed
        if removed > 0 {
            if let Err(e) = uploaded.store(&uploaded_path) {
                log::error!("Failed to save uploaded logs after cleanup: {}", e);
            } else {
                log::info!("Upload history cleanup complete: {} entries removed", removed);
            }
        }
    }

    // Load webhook settings at startup
    let webhooks_path = webhooks_path();
    if let Err(e) = WebhookSettings::from_path(&webhooks_path) {
        log::warn!("Failed to load webhook settings: {e}");
        // Only initialize and save if the file doesn't exist
        if !webhooks_path.exists() {
            log::info!("Webhook settings file doesn't exist, creating new one");
            let mut webhook_settings = WebhookSettings::get();
            webhook_settings.init();
            if let Err(e) = webhook_settings.store(&webhooks_path) {
                log::error!("Failed to save initialized webhook settings: {e}");
            }
        } else {
            log::error!("Webhook settings file exists but failed to parse - keeping in-memory defaults");
        }
    }

    // Load report history at startup
    let history_path = report_history_path();
    if let Err(e) = ReportHistory::from_path(&history_path) {
        log::warn!("Failed to load report history: {e}");
    }

    check_auto_cleanup_on_load();
    
    // Enable mouse lock if it was enabled last time
    let settings = Settings::get();
    if settings.mouse_lock_enabled {
        qol::enable_mouse_lock();
    }
    drop(settings);
    
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
        None::<&str>, // target_identifier: None means it appears in the main Nexus right-click menu
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

    qol::disable_mouse_lock();

    let settings = Settings::get();
    if let Err(e) = settings.store(config_path()) {
        log::error!("Failed to store settings: {e}");
    }
    drop(settings);

    // Save uploaded logs history
    let uploaded = UploadedLogs::get();
    if let Err(e) = uploaded.store(uploaded_logs_path()) {
        log::error!("Failed to store uploaded logs: {e}");
    }
    drop(uploaded);

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
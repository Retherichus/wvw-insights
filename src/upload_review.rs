use nexus::imgui::{ChildWindow, Ui};

use crate::settings::Settings;
use crate::state::{ProcessingState, STATE};
use crate::upload;

#[derive(Debug, Clone)]
pub struct UploadedFileInfo {
    pub filename: String,
    pub size: String,
    pub metadata: Option<FileMetadata>,
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub map_abbr: String,
    pub map_color: [f32; 4],
    pub recorder: Option<String>,
    pub commander: Option<String>,
    pub timestamp: Option<String>,
}

/// Renders the upload review screen where users can see uploaded files and decide what to do
pub fn render_upload_review(ui: &Ui) {
    let uploaded_files = STATE.uploaded_files.lock().unwrap().clone();
    let state = *STATE.processing_state.lock().unwrap();
    
    ui.text("Files uploaded to session:");
    ui.spacing();
    
    // Show uploaded files in a scrollable list
    ChildWindow::new("UploadedFilesList")
        .size([0.0, 350.0])
        .movable(false)
        .build(ui, || {
            if uploaded_files.is_empty() {
                ui.text_colored([0.7, 0.7, 0.7, 1.0], "No files uploaded yet");
            } else {
                for file in uploaded_files.iter() {
                    render_uploaded_file_item(ui, file);
                }
            }
        });
    
    ui.separator();
    
    let file_count = uploaded_files.len();
    ui.text(format!("Total files: {}", file_count));
    
    ui.spacing();
    
    // Action buttons
    if state != ProcessingState::Processing {
        // Start Processing button (only if files uploaded)
        if file_count > 0 {
            if ui.button("Start Processing") {
                log::info!("Starting processing for {} files", file_count);
                std::thread::spawn(|| {
                    start_processing_wrapper();
                });
            }
        } else {
            let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
            ui.button("Start Processing");
        }
        
        ui.same_line();
        
        // Upload More button
        if ui.button("Upload More Logs") {
            log::info!("Returning to log selection to upload more files");
            
            // Reset log selection states
            let mut logs = STATE.logs.lock().unwrap();
            for log in logs.iter_mut() {
                log.selected = false;
                // Don't reset uploaded or status - they stay as is
            }
            drop(logs);
            
            *STATE.show_upload_review.lock().unwrap() = false;
            *STATE.show_log_selection.lock().unwrap() = true;
        }
        
        ui.spacing();
        ui.separator();
        ui.spacing();
        
        // Cancel button - simplified, no popup
        if ui.button("Cancel") {
            log::info!("User cancelled upload session");
            std::thread::spawn(|| {
                clear_session();
                *STATE.show_upload_review.lock().unwrap() = false;
                *STATE.show_token_input.lock().unwrap() = true;
            });
        }
    } else {
        ui.text_colored([1.0, 1.0, 0.0, 1.0], "Processing in progress...");
    }
}

/// Renders a single uploaded file item with delete button
fn render_uploaded_file_item(ui: &Ui, file: &UploadedFileInfo) {
    let line_height = ui.text_line_height_with_spacing();
    let item_height = line_height * 2.5;
    
    let item_pos = ui.cursor_screen_pos();
    let content_width = ui.content_region_avail()[0];
    
    // Background
    let draw_list = ui.get_window_draw_list();
    draw_list
        .add_rect(
            item_pos,
            [item_pos[0] + content_width, item_pos[1] + item_height],
            [0.2, 0.2, 0.2, 0.3]
        )
        .filled(true)
        .rounding(2.0)
        .build();
    
    // Filename
    ui.text(&file.filename);
    
    ui.same_line();
    
    // Metadata if available
    if let Some(ref meta) = file.metadata {
        // Map badge
        let map_color = meta.map_color;
        ui.text_colored(map_color, &format!("[{}]", meta.map_abbr));
        ui.same_line();
        
        // Timestamp
        if let Some(ref timestamp) = meta.timestamp {
            ui.text_colored([0.6, 0.6, 0.6, 1.0], timestamp);
            ui.same_line();
        }
    }
    
    // Size
    ui.text_colored([0.7, 0.7, 0.7, 1.0], &format!("({})", file.size));
    
    // Second line - metadata
    if let Some(ref meta) = file.metadata {
        ui.spacing();
        
        if let Some(ref recorder) = meta.recorder {
            ui.text_colored([0.7, 0.9, 1.0, 1.0], "Char:");
            ui.same_line();
            ui.text_colored([0.8, 0.8, 0.8, 1.0], recorder);
            ui.same_line();
        }
        
        if let Some(ref commander) = meta.commander {
            ui.text_colored([1.0, 0.8, 0.2, 1.0], "Cmd:");
            ui.same_line();
            ui.text_colored([1.0, 0.9, 0.6, 1.0], commander);
            ui.same_line();
        }
    }
    
    // Delete button on the right
    let button_width = 60.0;
    let cursor_x = ui.cursor_pos()[0];
    let available_width = ui.content_region_avail()[0];
    ui.set_cursor_pos([cursor_x + available_width - button_width, ui.cursor_pos()[1]]);
    
    let delete_id = format!("Delete##{}", file.filename);
    if ui.small_button(&delete_id) {
        log::info!("Deleting file: {}", file.filename);
        std::thread::spawn({
            let filename = file.filename.clone();
            move || {
                delete_uploaded_file(&filename);
            }
        });
    }
    
    ui.dummy([0.0, 5.0]);
}

/// Wrapper to start processing with proper state management
fn start_processing_wrapper() {
    let settings = Settings::get();
    let api_endpoint = settings.api_endpoint.clone();
    let history_token = settings.history_token.clone();
    let guild_name = settings.guild_name.clone();
    let enable_legacy = settings.enable_legacy_parser;
    drop(settings);
    
    let session_id = STATE.session_id.lock().unwrap().clone();
    let ownership_token = STATE.ownership_token.lock().unwrap().clone();
    
    if session_id.is_empty() || ownership_token.is_empty() {
        log::error!("Cannot start processing: session not initialized");
        return;
    }
    
    // Reset timer state for new processing session
    *STATE.processing_time_estimate.lock().unwrap() = None;
    *STATE.processing_time_estimate_start.lock().unwrap() = None;
    
    match upload::start_processing(
        &api_endpoint,
        &session_id,
        &history_token,
        &ownership_token,
        &guild_name,
        enable_legacy,
    ) {
        Ok(message) => {
            log::info!("Processing started: {}", message);
            *STATE.processing_state.lock().unwrap() = ProcessingState::Processing;
            *STATE.last_status_check.lock().unwrap() = Some(std::time::Instant::now());
            *STATE.show_upload_review.lock().unwrap() = false;
            *STATE.show_upload_progress.lock().unwrap() = true;
        }
        Err(e) => {
            log::error!("Failed to start processing: {}", e);
            *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
            *STATE.report_urls.lock().unwrap() = vec![format!("Server error: {}", e)];
            *STATE.show_upload_review.lock().unwrap() = false;
            *STATE.show_upload_progress.lock().unwrap() = true;
        }
    }
}

/// Deletes an uploaded file from the server session
fn delete_uploaded_file(filename: &str) {
    let settings = Settings::get();
    let api_endpoint = settings.api_endpoint.clone();
    drop(settings);
    
    let session_id = STATE.session_id.lock().unwrap().clone();
    
    if session_id.is_empty() {
        log::error!("Cannot delete file: no active session");
        return;
    }
    
    match upload::delete_file(&api_endpoint, &session_id, filename) {
        Ok(message) => {
            log::info!("File deleted: {}", message);
            
            // Remove from local tracking
            let mut uploaded_files = STATE.uploaded_files.lock().unwrap();
            uploaded_files.retain(|f| f.filename != filename);
            drop(uploaded_files);
            
            // Also update the log status
            let mut logs = STATE.logs.lock().unwrap();
            if let Some(log) = logs.iter_mut().find(|l| l.filename == filename) {
                log.uploaded = false;
                log.status = "Ready".to_string();
            }
        }
        Err(e) => {
            log::error!("Failed to delete file: {}", e);
        }
    }
}

/// Clears the current session
fn clear_session() {
    *STATE.session_id.lock().unwrap() = String::new();
    *STATE.ownership_token.lock().unwrap() = String::new();
    *STATE.uploaded_files.lock().unwrap() = Vec::new();
    *STATE.processing_state.lock().unwrap() = ProcessingState::Idle;
    
    // Reset all log statuses
    let mut logs = STATE.logs.lock().unwrap();
    for log in logs.iter_mut() {
        if log.uploaded && !log.status.starts_with("Failed") {
            log.uploaded = false;
            log.status = "Ready".to_string();
        }
    }
}
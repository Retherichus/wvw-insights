use nexus::imgui::{ChildWindow, Ui};

use crate::formatting::{format_timestamp};
use crate::scanning::scan_for_logs;
use crate::settings::Settings;
use crate::state::{ProcessingState, TimeFilter, STATE};
use crate::uploaded_logs::UploadedLogs;

/// Renders the log selection screen
pub fn render_log_selection(ui: &Ui) {
    let logs = STATE.logs.lock().unwrap();
    let scan_in_progress = *STATE.scan_in_progress.lock().unwrap();

    ui.text(format!("Select WvW logs to upload ({} found)", logs.len()));

    // Time filter selection
    ui.spacing();
    ui.text("Show logs from:");
    ui.spacing();

    let mut current_filter = *STATE.selected_time_filter.lock().unwrap();
    let filter_changed = {
        let mut changed = false;

        if ui.radio_button(
            "This session",
            &mut current_filter,
            TimeFilter::SincePluginStart,
        ) {
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
        changed
    };

    ui.spacing();

    // Checkbox to show/hide previously uploaded logs
    let mut show_uploaded = *STATE.show_uploaded_logs.lock().unwrap();
    if ui.checkbox("Show previously uploaded logs", &mut show_uploaded) {
        *STATE.show_uploaded_logs.lock().unwrap() = show_uploaded;
    }

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

    drop(logs);

    // Apply filter change
    if filter_changed {
        *STATE.selected_time_filter.lock().unwrap() = current_filter;
        scan_for_logs();
        return;
    }

    let mut logs = STATE.logs.lock().unwrap();

    ui.separator();

    // Show scanning indicator or empty message
    if scan_in_progress && logs.is_empty() {
        ui.text_colored(
            [0.7, 0.9, 1.0, 1.0],
            "Scanning for logs...",
        );
        ui.spacing();

        if ui.button("Open Settings") {
            *STATE.show_log_selection.lock().unwrap() = false;
            *STATE.show_settings.lock().unwrap() = true;
            return;
        }

        ui.same_line();

        if ui.button("Back") {
            std::thread::spawn(|| {
                handle_back_navigation();
            });
            return;
        }

        return;
    }

    if logs.is_empty() && !scan_in_progress {
        ui.text_colored(
            [1.0, 0.0, 0.0, 1.0],
            "No WvW logs found with current filter!",
        );
        ui.spacing();

        if ui.button("Open Settings") {
            *STATE.show_log_selection.lock().unwrap() = false;
            *STATE.show_settings.lock().unwrap() = true;
            return;
        }

        ui.same_line();

        if ui.button("Back") {
            std::thread::spawn(|| {
                handle_back_navigation();
            });
            return;
        }

        return;
    }

    if scan_in_progress && !logs.is_empty() {
        ui.text_colored(
            [0.7, 0.9, 1.0, 1.0],
            "Scanning for new logs...",
        );
        ui.spacing();
    }

    // Selection buttons
    let show_select_all = matches!(
        current_filter,
        TimeFilter::SincePluginStart | TimeFilter::Last24Hours
    );

    if show_select_all {
        if ui.button("Select All") {
            let uploaded = UploadedLogs::get();
            let show_uploaded = *STATE.show_uploaded_logs.lock().unwrap();
            
            for log in logs.iter_mut() {
                if show_uploaded || !uploaded.is_uploaded(&log.filename) {
                    log.selected = true;
                }
            }
            drop(uploaded);
        }
        ui.same_line();
    } else {
        let _style = ui.push_style_color(nexus::imgui::StyleColor::Button, [0.3, 0.3, 0.3, 0.5]);
        let _style2 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonHovered, [0.3, 0.3, 0.3, 0.5]);
        let _style3 =
            ui.push_style_color(nexus::imgui::StyleColor::ButtonActive, [0.3, 0.3, 0.3, 0.5]);
        ui.button("Select All");
        if ui.is_item_hovered() {
            ui.tooltip_text("Only available for 'This session' and 'Last 24 hours' filters");
        }
        ui.same_line();
    }

    if ui.button("Deselect All") {
        for log in logs.iter_mut() {
            log.selected = false;
        }
    }

    ui.spacing();

    // Compact log list with better styling
    use nexus::imgui::MouseButton;
    ChildWindow::new("LogList")
        .size([0.0, 300.0])
        .movable(false)
        .build(ui, || {
            let draw_list = ui.get_window_draw_list();

            // Get window bounds
            let window_pos = ui.window_pos();
            let window_size = ui.window_size();
            let window_min = window_pos;
            let window_max = [window_pos[0] + window_size[0], window_pos[1] + window_size[1]];

            // Drag selection state
            static mut START_POS: Option<[f32; 2]> = None;
            static mut IS_DRAGGING: bool = false;
            static mut IS_DESELECT_DRAG: bool = false;
            static mut DRAG_STARTED: bool = false;
            
            const DRAG_THRESHOLD: f32 = 5.0;
            const SCROLL_ZONE: f32 = 40.0;
            const SCROLL_SPEED: f32 = 10.0;

            let mouse_pos = ui.io().mouse_pos;
            let left_clicked = ui.is_mouse_clicked(MouseButton::Left);
            let left_released = ui.is_mouse_released(MouseButton::Left);
            let right_clicked = ui.is_mouse_clicked(MouseButton::Right);
            let right_released = ui.is_mouse_released(MouseButton::Right);
            let left_down = ui.is_mouse_down(MouseButton::Left);
            let right_down = ui.is_mouse_down(MouseButton::Right);

            unsafe {
                if left_clicked && ui.is_window_hovered() {
                    START_POS = Some(mouse_pos);
                    IS_DRAGGING = false;
                    DRAG_STARTED = false;
                    IS_DESELECT_DRAG = false;
                }

                if right_clicked && ui.is_window_hovered() {
                    START_POS = Some(mouse_pos);
                    IS_DRAGGING = false;
                    DRAG_STARTED = false;
                    IS_DESELECT_DRAG = true;
                }

                if let Some(start) = START_POS {
                    if !DRAG_STARTED && (left_down || right_down) {
                        let dx = mouse_pos[0] - start[0];
                        let dy = mouse_pos[1] - start[1];
                        let distance = (dx * dx + dy * dy).sqrt();
                        
                        if distance > DRAG_THRESHOLD {
                            IS_DRAGGING = true;
                            DRAG_STARTED = true;
                        }
                    }
                }

                // Auto-scroll
                if IS_DRAGGING && DRAG_STARTED {
                    let scroll_y = ui.scroll_y();
                    let scroll_max_y = ui.scroll_max_y();
                    let relative_mouse_y = mouse_pos[1] - window_pos[1];
                    
                    if relative_mouse_y < SCROLL_ZONE && scroll_y > 0.0 {
                        ui.set_scroll_y((scroll_y - SCROLL_SPEED).max(0.0));
                    }
                    
                    if relative_mouse_y > (window_size[1] - SCROLL_ZONE) && scroll_y < scroll_max_y {
                        ui.set_scroll_y((scroll_y + SCROLL_SPEED).min(scroll_max_y));
                    }
                }

                if left_released || right_released {
                    IS_DRAGGING = false;
                    IS_DESELECT_DRAG = false;
                    DRAG_STARTED = false;
                    START_POS = None;
                }

                // Draw selection box
                if IS_DRAGGING && DRAG_STARTED {
                    if let Some(start) = START_POS {
                        let raw_rect_min = [start[0].min(mouse_pos[0]), start[1].min(mouse_pos[1])];
                        let raw_rect_max = [start[0].max(mouse_pos[0]), start[1].max(mouse_pos[1])];

                        let rect_min = [
                            raw_rect_min[0].max(window_min[0]),
                            raw_rect_min[1].max(window_min[1])
                        ];
                        let rect_max = [
                            raw_rect_max[0].min(window_max[0]),
                            raw_rect_max[1].min(window_max[1])
                        ];

                        let (fill_color, border_color) = if IS_DESELECT_DRAG {
                            ([1.0, 0.2, 0.2, 0.2], [1.0, 0.2, 0.2, 0.6])
                        } else {
                            ([0.2, 0.5, 1.0, 0.2], [0.2, 0.5, 1.0, 0.6])
                        };

                        draw_list
                            .add_rect(rect_min, rect_max, fill_color)
                            .filled(true)
                            .build();

                        draw_list
                            .add_rect(rect_min, rect_max, border_color)
                            .build();
                    }
                }
            }

            // Render compact log items
            let settings = Settings::get();
            let use_formatted = settings.show_formatted_timestamps;
            drop(settings);

            let uploaded = UploadedLogs::get();
            let show_uploaded = *STATE.show_uploaded_logs.lock().unwrap();

            for log in logs.iter_mut() {
                let is_uploaded = uploaded.is_uploaded(&log.filename);
                if is_uploaded && !show_uploaded {
                    continue;
                }
                
                // NEW: Skip logs already in current session
                let in_current_session = {
                    let uploaded_files = STATE.uploaded_files.lock().unwrap();
                    uploaded_files.iter().any(|f| f.filename == log.filename)
                };
                if in_current_session {
                    continue;
                }

                // Compact item height - single line with info on same line
                let line_height = ui.text_line_height_with_spacing();
                let item_height = line_height * 1.8; // Reduced from 2.5
                
                let item_screen_pos = ui.cursor_screen_pos();
                let content_width = ui.content_region_avail()[0];

                // Better background for uploaded logs - more visible
                if is_uploaded {
                    draw_list
                        .add_rect(
                            item_screen_pos,
                            [item_screen_pos[0] + content_width, item_screen_pos[1] + item_height],
                            [0.1, 0.4, 0.1, 0.3] // Increased alpha from 0.15 to 0.3
                        )
                        .filled(true)
                        .rounding(2.0)
                        .build();
                }

                // Draw selection highlight when selected
                if log.selected {
                    draw_list
                        .add_rect(
                            item_screen_pos,
                            [item_screen_pos[0] + content_width, item_screen_pos[1] + item_height],
                            [0.2, 0.5, 1.0, 0.2]
                        )
                        .filled(true)
                        .rounding(2.0)
                        .build();
                }

                // Check intersection with drag selection box
                unsafe {
                    if IS_DRAGGING && DRAG_STARTED {
                        if let Some(start) = START_POS {
                            let raw_rect_min = [start[0].min(mouse_pos[0]), start[1].min(mouse_pos[1])];
                            let raw_rect_max = [start[0].max(mouse_pos[0]), start[1].max(mouse_pos[1])];

                            let rect_min = [
                                raw_rect_min[0].max(window_min[0]),
                                raw_rect_min[1].max(window_min[1])
                            ];
                            let rect_max = [
                                raw_rect_max[0].min(window_max[0]),
                                raw_rect_max[1].min(window_max[1])
                            ];

                            let item_min = item_screen_pos;
                            let item_max = [item_screen_pos[0] + content_width, item_screen_pos[1] + item_height];

                            let overlaps = !(item_max[0] < rect_min[0]
                                || item_min[0] > rect_max[0]
                                || item_max[1] < rect_min[1]
                                || item_min[1] > rect_max[1]);

                            if overlaps {
                                log.selected = !IS_DESELECT_DRAG;
                            }
                        }
                    }
                }

                // Checkbox
                ui.checkbox(&format!("##checkbox_{}", log.filename), &mut log.selected);
                ui.same_line();

                // Single line layout - Date/Time
                if use_formatted {
                    if let Some(formatted) = format_timestamp(&log.filename) {
                        ui.text(&formatted);
                    } else {
                        ui.text(&log.filename);
                    }
                } else {
                    ui.text(&log.filename);
                }
                
                ui.same_line();
                
                // Map badge with color coding
                let map_name = log.map_type.display_name();
                let map_color = match log.map_type {
                    crate::logfile::MapType::EternalBattlegrounds => [0.8, 0.6, 0.2, 1.0],
                    crate::logfile::MapType::GreenAlpineBorderlands => [0.2, 0.8, 0.3, 1.0],
                    crate::logfile::MapType::BlueAlpineBorderlands => [0.3, 0.5, 1.0, 1.0],
                    crate::logfile::MapType::RedDesertBorderlands => [1.0, 0.3, 0.3, 1.0],
                    crate::logfile::MapType::EdgeOfTheMists => [0.6, 0.3, 0.8, 1.0],
                    crate::logfile::MapType::ObsidianSanctum => [0.4, 0.4, 0.4, 1.0],
                    _ => [0.5, 0.5, 0.5, 1.0],
                };
                
                ui.text_colored(map_color, &format!("[{}]", map_name));
                
                ui.same_line();
                
                // Recorder (only show if present)
                if let Some(ref recorder) = log.recorder {
                    ui.text_colored([0.7, 0.9, 1.0, 1.0], "Char:");
                    ui.same_line();
                    ui.text_colored([0.8, 0.8, 0.8, 1.0], recorder);
                    ui.same_line();
                }
                
                // Commander (only show if present)
                if let Some(ref commander) = log.commander {
                    ui.text_colored([1.0, 0.8, 0.2, 1.0], "Cmd:");
                    ui.same_line();
                    ui.text_colored([1.0, 0.9, 0.6, 1.0], commander);
                    ui.same_line();
                }
                
                // File size at the end
                ui.text_colored([0.6, 0.6, 0.6, 1.0], &format!("{:.1}MB", log.size as f64 / 1024.0 / 1024.0));

                // Add minimal spacing between items
                ui.dummy([0.0, 2.0]);
            }

            drop(uploaded);
        });
                
    ui.separator();

    let uploaded = UploadedLogs::get();
    let show_uploaded = *STATE.show_uploaded_logs.lock().unwrap();
    
    let selected_count = logs.iter().filter(|l| {
        let is_uploaded = uploaded.is_uploaded(&l.filename);
        l.selected && (show_uploaded || !is_uploaded)
    }).count();
    drop(uploaded);
    
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

        std::thread::spawn(|| {
            start_upload_process();
        });
    }

    ui.same_line();

    if ui.button("Back") {
        std::thread::spawn(|| {
            log::info!("Back button clicked from log selection");
            handle_back_navigation();
        });
    }
    
    let session_exists = !STATE.session_id.lock().unwrap().is_empty();
    let files_in_session = STATE.uploaded_files.lock().unwrap().len();

    if session_exists && files_in_session > 0 {
        ui.spacing();
        ui.separator();
        ui.spacing();
        
        ui.text_colored([1.0, 0.8, 0.2, 1.0], &format!("Active session: {} file(s) ready", files_in_session));
        
        if ui.button("Go to Review & Process") {
            log::info!("Navigating to review screen");
            *STATE.show_log_selection.lock().unwrap() = false;
            *STATE.show_upload_review.lock().unwrap() = true;
        }
    }    
}

/// Handles back navigation logic based on session state
fn handle_back_navigation() {
    // Check if we have an active session with uploads
    let has_uploads = !STATE.uploaded_files.lock().unwrap().is_empty();
    
    *STATE.show_log_selection.lock().unwrap() = false;
    
    if has_uploads {
        // Go back to review screen (session preserved)
        log::info!("Returning to upload review (session with {} files preserved)", 
            STATE.uploaded_files.lock().unwrap().len());
        *STATE.show_upload_review.lock().unwrap() = true;
    } else {
        // No active session or no uploads - clear everything and go to token input
        log::info!("No uploads in session, clearing session and returning to token input");
        STATE.session_id.lock().unwrap().clear();
        STATE.ownership_token.lock().unwrap().clear();
        *STATE.show_token_input.lock().unwrap() = true;
    }
}

/// Starts the upload process for selected logs
fn start_upload_process() {
    log::info!("Starting upload process");

    *STATE.processing_state.lock().unwrap() = ProcessingState::Uploading;

    let settings = Settings::get();
    let api_endpoint = settings.api_endpoint.clone();
    let history_token = settings.history_token.clone();
    drop(settings);

    // Check if we have an existing session or need to create one
    let session_id = {
        let existing_session = STATE.session_id.lock().unwrap().clone();
        if !existing_session.is_empty() {
            log::info!("Using existing session: {}", existing_session);
            existing_session
        } else {
            // Create new session
            log::info!("Creating new session");
            match crate::upload::create_session(&api_endpoint, &history_token) {
                Ok((sid, ot)) => {
                    log::info!("Session created: {}", sid);
                    *STATE.session_id.lock().unwrap() = sid.clone();
                    *STATE.ownership_token.lock().unwrap() = ot.clone();
                    sid
                }
                Err(e) => {
                    log::error!("Failed to create session: {}", e);
                    *STATE.processing_state.lock().unwrap() = ProcessingState::Failed;
                    return;
                }
            }
        }
    };

    // Get selected logs
    let selected_logs: Vec<(usize, crate::logfile::LogFile)> = {
        let logs = STATE.logs.lock().unwrap();
        logs.iter()
            .enumerate()
            .filter(|(_, log)| log.selected)
            .map(|(i, log)| (i, log.clone()))
            .collect()
    };
    
    // APPEND to uploaded_files (don't clear if session already exists)
    {
        let mut uploaded_files = STATE.uploaded_files.lock().unwrap();
        
        for (_, log) in selected_logs.iter() {
            use crate::upload_review::{UploadedFileInfo, FileMetadata};
            use crate::formatting::format_timestamp;
            
            // Check if already in list
            if uploaded_files.iter().any(|f| f.filename == log.filename) {
                continue;
            }
                        
            uploaded_files.push(UploadedFileInfo {
                filename: log.filename.clone(),
                size: format!("{:.2} MB", log.size as f64 / 1024.0 / 1024.0),
                metadata: Some(FileMetadata {
                    map_abbr: log.map_type.display_name().to_string(),
                    map_color: get_map_color(&log.map_type),
                    recorder: log.recorder.clone(),
                    commander: log.commander.clone(),
                    timestamp: format_timestamp(&log.filename),
                }),
            });
        }
        
        log::info!("uploaded_files now has {} entries", uploaded_files.len());
    }
    
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

// Helper function to get map colors
fn get_map_color(map_type: &crate::logfile::MapType) -> [f32; 4] {
    use crate::logfile::MapType;
    match map_type {
        MapType::EternalBattlegrounds => [0.8, 0.6, 0.2, 1.0],
        MapType::GreenAlpineBorderlands => [0.2, 0.8, 0.3, 1.0],
        MapType::BlueAlpineBorderlands => [0.3, 0.5, 1.0, 1.0],
        MapType::RedDesertBorderlands => [1.0, 0.3, 0.3, 1.0],
        MapType::EdgeOfTheMists => [0.6, 0.3, 0.8, 1.0],
        MapType::ObsidianSanctum => [0.4, 0.4, 0.4, 1.0],
        _ => [0.5, 0.5, 0.5, 1.0],
    }
}
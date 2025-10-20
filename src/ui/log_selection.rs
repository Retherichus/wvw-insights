use nexus::imgui::{ChildWindow, Ui};

use crate::formatting::format_timestamp;
use crate::scanning::scan_for_logs;
use crate::settings::Settings;
use crate::state::{ProcessingState, TimeFilter, STATE};
use crate::uploaded_logs::UploadedLogs;

/// Renders the log selection screen
pub fn render_log_selection(ui: &Ui) {
    let logs = STATE.logs.lock().unwrap();

    ui.text(format!("Select logs to upload ({} found)", logs.len()));

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

        // "Show Everything" radio button that triggers warning
        if ui.radio_button("Show Everything", &mut current_filter, TimeFilter::AllLogs) {
            // Don't apply change yet, show warning popup first
            ui.open_popup("load_all_warning");
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
            std::thread::spawn(|| {
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
            let uploaded = UploadedLogs::get();
            let show_uploaded = *STATE.show_uploaded_logs.lock().unwrap();
            
            for log in logs.iter_mut() {
                // Only select if we're showing uploaded logs, or if it's not uploaded
                if show_uploaded || !uploaded.is_uploaded(&log.filename) {
                    log.selected = true;
                }
            }
            drop(uploaded);
        }
        ui.same_line();
    } else {
        // Show disabled Select All button with tooltip for other filters
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

    // Deselect All always works
    if ui.button("Deselect All") {
        for log in logs.iter_mut() {
            log.selected = false;
        }
    }

    ui.spacing();

    use nexus::imgui::MouseButton;
    ChildWindow::new("LogList")
        .size([0.0, 300.0])
        .movable(false)
        .build(ui, || {
            let draw_list = ui.get_window_draw_list();
            let item_height = ui.text_line_height_with_spacing();

            // --- DRAG SELECTION STATE ---
            static mut START_POS: Option<[f32; 2]> = None;
            static mut IS_DRAGGING: bool = false;
            static mut IS_DESELECT_DRAG: bool = false;
            static mut DRAG_STARTED: bool = false; // NEW: Track if we've actually started dragging
            
            // Drag threshold in pixels - adjust this value to tune sensitivity
            const DRAG_THRESHOLD: f32 = 5.0;

            let mouse_pos = ui.io().mouse_pos;
            let left_clicked = ui.is_mouse_clicked(MouseButton::Left);
            let left_released = ui.is_mouse_released(MouseButton::Left);
            let right_clicked = ui.is_mouse_clicked(MouseButton::Right);
            let right_released = ui.is_mouse_released(MouseButton::Right);
            let left_down = ui.is_mouse_down(MouseButton::Left);
            let right_down = ui.is_mouse_down(MouseButton::Right);

            unsafe {
                // Start potential drag when user clicks inside this child
                if left_clicked && ui.is_window_hovered() {
                    START_POS = Some(mouse_pos);
                    IS_DRAGGING = false; // Don't mark as dragging yet
                    DRAG_STARTED = false;
                    IS_DESELECT_DRAG = false;
                }

                // Start potential deselect drag on right click
                if right_clicked && ui.is_window_hovered() {
                    START_POS = Some(mouse_pos);
                    IS_DRAGGING = false; // Don't mark as dragging yet
                    DRAG_STARTED = false;
                    IS_DESELECT_DRAG = true;
                }

                // Check if we've moved enough to start actual dragging
                if let Some(start) = START_POS {
                    if !DRAG_STARTED && (left_down || right_down) {
                        let dx = mouse_pos[0] - start[0];
                        let dy = mouse_pos[1] - start[1];
                        let distance = (dx * dx + dy * dy).sqrt();
                        
                        // Only start dragging if we've moved beyond the threshold
                        if distance > DRAG_THRESHOLD {
                            IS_DRAGGING = true;
                            DRAG_STARTED = true;
                        }
                    }
                }

                // Stop drag when mouse released
                if left_released || right_released {
                    IS_DRAGGING = false;
                    IS_DESELECT_DRAG = false;
                    DRAG_STARTED = false;
                    START_POS = None;
                }

                // --- DRAW SELECTION BOX & CHECK INTERSECTIONS ---
                if IS_DRAGGING && DRAG_STARTED {
                    if let Some(start) = START_POS {
                        let rect_min = [start[0].min(mouse_pos[0]), start[1].min(mouse_pos[1])];
                        let rect_max = [start[0].max(mouse_pos[0]), start[1].max(mouse_pos[1])];

                        // Different colors for select vs deselect
                        let (fill_color, border_color) = if IS_DESELECT_DRAG {
                            ([1.0, 0.2, 0.2, 0.2], [1.0, 0.2, 0.2, 0.6]) // Red for deselect
                        } else {
                            ([0.2, 0.5, 1.0, 0.2], [0.2, 0.5, 1.0, 0.6]) // Blue for select
                        };

                        // Draw translucent filled selection box
                        draw_list
                            .add_rect(rect_min, rect_max, fill_color)
                            .filled(true)
                            .build();

                        // Outline
                        draw_list
                            .add_rect(rect_min, rect_max, border_color)
                            .build();
                    }
                }
            }

            // --- NORMAL LOG RENDERING ---
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

                // Get the screen position BEFORE rendering the item
                let item_screen_pos = ui.cursor_screen_pos();
                let content_width = ui.content_region_avail()[0];

                // Background highlight for uploaded logs
                if is_uploaded {
                    draw_list
                        .add_rect(
                            item_screen_pos,
                            [item_screen_pos[0] + content_width, item_screen_pos[1] + item_height],
                            [0.0, 0.3, 0.0, 0.3]
                        )
                        .filled(true)
                        .build();
                }

                // Check if this item intersects with the selection box
                // ONLY if we've actually started dragging (moved beyond threshold)
                unsafe {
                    if IS_DRAGGING && DRAG_STARTED {
                        if let Some(start) = START_POS {
                            let rect_min = [start[0].min(mouse_pos[0]), start[1].min(mouse_pos[1])];
                            let rect_max = [start[0].max(mouse_pos[0]), start[1].max(mouse_pos[1])];

                            let item_min = item_screen_pos;
                            let item_max = [item_screen_pos[0] + content_width, item_screen_pos[1] + item_height];

                            let overlaps = !(item_max[0] < rect_min[0]
                                || item_min[0] > rect_max[0]
                                || item_max[1] < rect_min[1]
                                || item_min[1] > rect_max[1]);

                            if overlaps {
                                // Select or deselect based on drag type
                                log.selected = !IS_DESELECT_DRAG;
                            }
                        }
                    }
                }

                // Checkbox and text
                ui.checkbox(&format!("##checkbox_{}", log.filename), &mut log.selected);
                ui.same_line();

                if use_formatted {
                    if let Some(formatted) = format_timestamp(&log.filename) {
                        ui.text(&formatted);
                        ui.same_line();
                        ui.text_colored(
                            [0.7, 0.7, 0.7, 1.0],
                            &format!("({:.2} MB)", log.size as f64 / 1024.0 / 1024.0),
                        );
                    } else {
                        ui.text(&log.filename);
                        ui.same_line();
                        ui.text(format!("({:.2} MB)", log.size as f64 / 1024.0 / 1024.0));
                    }
                } else {
                    ui.text(&log.filename);
                    ui.same_line();
                    ui.text(format!("({:.2} MB)", log.size as f64 / 1024.0 / 1024.0));
                }
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
            *STATE.show_log_selection.lock().unwrap() = false;
            *STATE.show_token_input.lock().unwrap() = true;
        });
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

    // Create session
    log::info!("Creating session");
    let (session_id, _ownership_token) =
        match crate::upload::create_session(&api_endpoint, &history_token) {
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
    let selected_logs: Vec<(usize, crate::logfile::LogFile)> = {
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
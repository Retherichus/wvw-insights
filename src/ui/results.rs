use nexus::imgui::Ui;

use crate::scanning::scan_for_logs;
use crate::state::STATE;
use crate::ui::upload_progress::reset_upload_state;
use crate::uploaded_logs::UploadedLogs;

/// Renders the results screen after processing is complete
pub fn render_results(ui: &Ui) {
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
        std::thread::spawn(|| {
            log::info!("Resetting upload state");
            
            // Mark uploaded logs BEFORE resetting state
            mark_uploaded_logs();
            
            reset_upload_state();
            log::info!("State reset complete, starting log scan");
            scan_for_logs();
            log::info!("Log scan complete");
        });
    }

    ui.same_line();

    if ui.button("Back to Start") {
        std::thread::spawn(|| {
            log::info!("Back to Start button clicked");
            
            // Mark uploaded logs BEFORE resetting state
            mark_uploaded_logs();
            
            reset_upload_state();
            *STATE.show_log_selection.lock().unwrap() = false;
            *STATE.show_token_input.lock().unwrap() = true;
        });
    }
}

/// Marks successfully uploaded logs in the uploaded logs tracker
fn mark_uploaded_logs() {
    let logs = STATE.logs.lock().unwrap();
    let mut uploaded = UploadedLogs::get();
    
    let mut newly_added = 0;
    for log in logs.iter() {
        if log.selected && log.uploaded {
            if !uploaded.is_uploaded(&log.filename) {
                uploaded.add_log(log.filename.clone());
                newly_added += 1;
            }
        }
    }
    
    if newly_added > 0 {
        log::info!("Marked {} new logs as uploaded", newly_added);
        
        // Save to disk
        if let Err(e) = uploaded.store(crate::uploaded_logs_path()) {
            log::error!("Failed to save uploaded logs: {}", e);
        }
    }
    
    drop(uploaded);
    drop(logs);
}
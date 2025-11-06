use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use crate::common::WorkerMessage;

pub type UploadJob = (usize, PathBuf, String, String, String);

thread_local! {
    static CLIENT: ureq::Agent = ureq::agent()
}

// Legacy overhead multiplier - MUST match JS version
const LEGACY_INITIAL_MULTIPLIER: f32 = 2.00;
thread_local! {
    static HIGHEST_PROGRESS: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
}

#[derive(Debug, Deserialize)]
struct SessionResponse {
    success: bool,
    session_id: Option<String>,
    ownership_token: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UploadResponse {
    success: bool,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeleteResponse {
    success: bool,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    status: String,
    progress: Option<f32>,
    #[allow(dead_code)]
    logs: Option<Vec<LogEntry>>,
    files: Option<Vec<FileEntry>>,
    heartbeat: Option<Heartbeat>,
    // Queue support fields
    queue_position: Option<i32>,
    avg_service_time: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct LogEntry {
    message: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    log_type: String,
}

#[derive(Debug, Deserialize)]
struct FileEntry {
    name: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct Heartbeat {
    component: Option<String>,
}

pub fn create_session(api_endpoint: &str, history_token: &str) -> Result<(String, String)> {  // REMOVE dps_report_token parameter
    let url = format!("{}?endpoint=nexus-session", api_endpoint);
    
    let response = CLIENT.with(|c| {
        c.post(&url).send_form(&[
            ("history_token", history_token),
        ])
    })?;

    let session: SessionResponse = response.into_json()?;
    
    log::info!("Session creation response: {:?}", session);
    
    if session.success {
        let session_id = session.session_id.ok_or_else(|| anyhow!("No session_id in response"))?;
        let ownership_token = session.ownership_token.ok_or_else(|| anyhow!("No ownership_token in response"))?;
        Ok((session_id, ownership_token))
    } else {
        Err(anyhow!("Session creation failed: {}", session.message.unwrap_or_default()))
    }
}

pub fn run(
    inc: Receiver<UploadJob>,
    out: Sender<WorkerMessage>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("wvw-insights-thread".to_string())
        .spawn(move || {
            for (index, location, api_endpoint, session_id, history_token) in inc {
                log::info!("Uploading {:?}", location);
                
                let result = upload_file(location, &api_endpoint, &session_id, &history_token);
                
                if let Err(e) = out.send(WorkerMessage::upload_result(index, result)) {
                    log::error!("Failed to send upload result: {e}");
                }
            }
        })
        .expect("Could not create upload thread")
}

fn upload_file(
    location: PathBuf,
    api_endpoint: &str,
    session_id: &str,
    history_token: &str,
) -> Result<String> {
    log::info!("Uploading {}", location.display());

    let url = format!("{}?endpoint=nexus-upload", api_endpoint);

    CLIENT.with(|c| {
        let (content_type, data) = ureq_multipart::MultipartBuilder::new()
            .add_text("session_id", session_id)?
            .add_text("history_token", history_token)?
            .add_file("file", &location)?
            .finish()?;
        
        let response = c
            .post(&url)
            .set("Content-Type", &content_type)
            .send_bytes(&data)?;

        let upload_resp: UploadResponse = response.into_json()?;
        
        if upload_resp.success {
            Ok("Uploaded".to_string())
        } else {
            Err(anyhow!("Upload failed: {}", upload_resp.message.unwrap_or_default()))
        }
    })
}

pub fn delete_file(
    api_endpoint: &str,
    session_id: &str,
    filename: &str,
) -> Result<String> {
    log::info!("Deleting file: {} from session: {}", filename, session_id);

    let url = format!("{}?endpoint=delete-upload", api_endpoint);

    CLIENT.with(|c| {
        let response = c
            .post(&url)
            .send_form(&[
                ("session_id", session_id),
                ("filename", filename),
            ])?;

        let delete_resp: DeleteResponse = response.into_json()?;
        
        if delete_resp.success {
            let msg = delete_resp.message.unwrap_or_else(|| "File deleted".to_string());
            log::info!("Delete successful: {}", msg);
            Ok(msg)
        } else {
            let error = delete_resp.message.unwrap_or_else(|| "Unknown error".to_string());
            Err(anyhow!("Delete failed: {}", error))
        }
    })
}

pub fn start_processing(
    api_endpoint: &str,
    session_id: &str,
    history_token: &str,
    ownership_token: &str,
    guild_name: &str,
    enable_legacy_parser: bool,
    dps_report_token: &str,
) -> Result<String> {
    let url = format!("{}?endpoint=nexus-process", api_endpoint);
    
    let final_guild_name = if guild_name.trim().is_empty() {
        "WvW Insights Parser (Nexus)"
    } else {
        guild_name
    };
    
    let legacy_parser_value = if enable_legacy_parser { "1" } else { "0" };
    
    let response = CLIENT.with(|c| {
        // Build form data dynamically to conditionally include dps_report_token
        let mut form_data = vec![
            ("session_id", session_id),
            ("history_token", history_token),
            ("ownership_token", ownership_token),
            ("guild_name", final_guild_name),
            ("enable_old_parser", legacy_parser_value),
        ];
        
        // Only include dps_report_token if it's not empty
        if !dps_report_token.is_empty() {
            form_data.push(("dps_report_token", dps_report_token));
        }
        
        c.post(&url).send_form(&form_data)
    })?;

    let resp: serde_json::Value = response.into_json()?;
    
    log::info!("Processing API response: {:?}", resp);
    
    if resp["success"].as_bool().unwrap_or(false) {
        let message = resp["message"].as_str().unwrap_or("Processing started").to_string();
        Ok(message)
    } else {
        let error_msg = resp["message"].as_str().unwrap_or("Processing start failed");
        Err(anyhow!("{}", error_msg))
    }
}

pub fn check_status(api_endpoint: &str, session_id: &str) -> Result<(String, Option<Vec<String>>, f32, Option<String>)> {
    let url = format!("{}?endpoint=process-status&session_id={}", api_endpoint, session_id);
    
    let response = CLIENT.with(|c| c.get(&url).call())?;
    let status_resp: StatusResponse = response.into_json()?;
    
    log::info!("Status: {} - Progress: {:?}", status_resp.status, status_resp.progress);
    
    // Handle queued status
    if status_resp.status == "queued" {
        let position = status_resp.queue_position.unwrap_or(0);
        let per_user_minutes = status_resp.avg_service_time.unwrap_or(1.0);
        let estimated_minutes = (position as f32 * per_user_minutes).round() as i32;
        
        let wait_text = if position <= 0 {
            format!("Starting soon (~{:.0} minute)", per_user_minutes)
        } else if estimated_minutes == 1 {
            "Estimated wait: ~1 minute".to_string()
        } else {
            format!("Estimated wait: ~{} minutes", estimated_minutes)
        };
        
        let phase = Some(format!(
            "Queued for processing (Position: {}) - {} — typically ~{:.0} minute per user",
            position, wait_text, per_user_minutes
        ));
        
        log::info!("In queue at position {} - estimated wait: {} minutes", position, estimated_minutes);
        
        // Return queued status with 0% progress and the queue message
        return Ok((status_resp.status, None, 0.0, phase));
    }
    
    let raw_progress = status_resp.progress.unwrap_or(0.0);
    
    // Get current phase from heartbeat component
    let current_component = status_resp.heartbeat
        .as_ref()
        .and_then(|h| h.component.as_ref())
        .map(|s| s.as_str());
    
    let progress = HIGHEST_PROGRESS.with(|highest| {
        let current_highest = highest.get();
        if raw_progress > current_highest {
            highest.set(raw_progress);
            raw_progress
        } else {
            current_highest
        }
    });
    
    log::info!("Progress: raw={:.1}%, display={:.1}%", raw_progress, progress);
    
    // Get legacy parser setting from STATE (do this ONCE at the start)
    let settings = crate::settings::Settings::get();
    let enable_legacy_parser = settings.enable_legacy_parser;
    drop(settings);
    
    // Check if we've already set initial estimate by checking STATE instead of thread_local
    let mut has_set_initial = crate::state::STATE.processing_time_estimate.lock().unwrap().is_some();
    
    // Process logs for time estimates (mirroring JS logic)
    if let Some(ref logs) = status_resp.logs {
        for log in logs.iter() {
            let msg = &log.message;
            
            // Extract initial TopStats estimate
            let topstats_estimate = extract_time_estimate_from_log(msg);
            
            // Extract TopStats completion time
            let topstats_completion = extract_completion_time_from_log(msg);
            
            // Initial Total Estimate (mirroring JS)
            if let Some(estimate) = topstats_estimate {
                if !has_set_initial {
                    has_set_initial = true;
                    
                    let total_estimate = if enable_legacy_parser {
                        let legacy_add = (estimate as f32 * LEGACY_INITIAL_MULTIPLIER).round() as u32;
                        let total = estimate + legacy_add;
                        log::info!("Initial estimate: TopStats {}s + Legacy {}s = {}s total", 
                                 estimate, legacy_add, total);
                        total
                    } else {
                        log::info!("Initial estimate: TopStats only {}s", estimate);
                        estimate
                    };
                    
                    *crate::state::STATE.processing_time_estimate.lock().unwrap() = Some(total_estimate);
                    *crate::state::STATE.processing_time_estimate_start.lock().unwrap() = Some(std::time::Instant::now());
                }
            }
            
            // Update Timer When TopStats Actually Completes (mirroring JS)
            if let Some(completion_time) = topstats_completion {
                if enable_legacy_parser && has_set_initial {
                    let current_estimate = *crate::state::STATE.processing_time_estimate.lock().unwrap();
                    let new_remaining = (completion_time as f32 * LEGACY_INITIAL_MULTIPLIER).round() as u32;
                    
                    // Only update if we haven't already updated to the legacy-only time
                    // Check if current estimate is significantly different from new_remaining
                    if let Some(current) = current_estimate {
                        if (current as i32 - new_remaining as i32).abs() > 10 {
                            log::info!("TopStats done in {}s → updating remaining to Legacy only: ~{}s (old total: {}s)", 
                                     completion_time, new_remaining, current);
                            
                            *crate::state::STATE.processing_time_estimate.lock().unwrap() = Some(new_remaining);
                            *crate::state::STATE.processing_time_estimate_start.lock().unwrap() = Some(std::time::Instant::now());
                        }
                    }
                }
            }
        }
    }
    
    // Get current phase message
    let phase = current_component.map(|c| {
        // ONLY clear timer on actual completion/failure
        let should_clear = matches!(c, "complete" | "failed");
        
        if should_clear {
            let current_estimate = *crate::state::STATE.processing_time_estimate.lock().unwrap();
            if current_estimate.is_some() {
                log::info!("Phase {} - clearing timer (final state)", c);
                *crate::state::STATE.processing_time_estimate.lock().unwrap() = None;
                *crate::state::STATE.processing_time_estimate_start.lock().unwrap() = None;
            }
        }
        
        get_phase_message(&c, progress)
    });
    
    let report_urls = if status_resp.status == "complete" {
        status_resp.files
            .map(|files| {
                files.iter()
                    .filter_map(|f| {
                        if f.name.contains("Report.html") || f.name.contains("LegacyReport.html") {
                            Some(f.url.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            })
    } else {
        None
    };
    
    Ok((status_resp.status, report_urls, progress, phase))
}

fn extract_time_estimate_from_log(message: &str) -> Option<u32> {
    let lower = message.to_lowercase();
    
    // Require BOTH json.gz and "estimated processing time" (matching JS)
    if lower.contains("json.gz") && lower.contains("estimated processing time") {
        // Minutes format (with decimals allowed)
        if let Some(min_match) = extract_decimal_value(&lower, "estimated processing time:", "minute") {
            return Some((min_match * 60.0).round() as u32);
        }
        
        // Seconds format
        if let Some(sec_match) = extract_integer_value(&lower, "estimated processing time:", "second") {
            return Some(sec_match);
        }
    }
    
    None
}

fn extract_completion_time_from_log(message: &str) -> Option<u32> {
    let lower = message.to_lowercase();
    
    // Match ONLY TopStats completion (not TW5/Legacy parser)
    if lower.contains("topstats completed successfully in") {
        if let Some(sec_match) = extract_decimal_value(&lower, "completed successfully in", "second") {
            return Some(sec_match.round() as u32);
        }
    }
    
    None
}

// Helper to extract decimal values from text
fn extract_decimal_value(text: &str, prefix: &str, suffix: &str) -> Option<f32> {
    if let Some(prefix_pos) = text.find(prefix) {
        let after_prefix = &text[prefix_pos + prefix.len()..];
        
        if let Some(suffix_pos) = after_prefix.find(suffix) {
            let between = &after_prefix[..suffix_pos].trim();
            
            // Extract decimal number (digits and dots)
            let number: String = between.chars()
                .filter(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            
            if !number.is_empty() {
                if let Ok(value) = number.parse::<f32>() {
                    return Some(value);
                }
            }
        }
    }
    
    None
}

// Helper to extract integer values from text
fn extract_integer_value(text: &str, prefix: &str, suffix: &str) -> Option<u32> {
    if let Some(prefix_pos) = text.find(prefix) {
        let after_prefix = &text[prefix_pos + prefix.len()..];
        
        if let Some(suffix_pos) = after_prefix.find(suffix) {
            let between = &after_prefix[..suffix_pos].trim();
            
            // Extract integer number
            let number: String = between.chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            
            if !number.is_empty() {
                if let Ok(value) = number.parse::<u32>() {
                    return Some(value);
                }
            }
        }
    }
    
    None
}

fn get_phase_message(component: &str, progress: f32) -> String {
    // Handle Elite Insights file progress
    if component.starts_with("elite_insights_processing_") {
        let parts: Vec<&str> = component.split('_').collect();
        if parts.len() >= 5 {
            if let (Ok(current), Ok(total)) = (parts[3].parse::<i32>(), parts[4].parse::<i32>()) {
                return format!("Processing logs with Elite Insights ({}/{})", current, total);
            }
        }
        return "Processing log data with Elite Insights".to_string();
    }
    
    match component {
        // Regular processing components
        "initialization" => "Initializing processing environment",
        "config_verification" => "Verifying configuration files",
        "elite_insights_start" => "Starting Elite Insights analysis",
        "elite_insights_executing" => "Running Elite Insights CLI",
        "elite_insights_processing" => "Processing log data with Elite Insights",
        "elite_insights_complete" => "Elite Insights processing completed",
        "topstats_start" => "Starting TopStats statistical analysis",
        "topstats_parsing" => "Parsing combat data with TopStats",
        "topstats_processing" => "Analyzing player performance metrics",
        "topstats_file_processing" => "Processing combat log files",
        "topstats_document_creation" => "Generating statistical documents",
        "topstats_complete" => "Finalizing combat statistics",
        "json_processing" => "Processing JSON combat data",
        "highscores_injection" => "Injecting high scores data",
        "tiddlywiki_start" => "Starting TiddlyWiki report generation",
        "tiddlywiki_initializing" => "Initializing TiddlyWiki report engine",
        "tiddlywiki_setup" => "Setting up wiki environment",
        "tiddlywiki_init" => "Initializing wiki workspace",
        "tiddlywiki_import" => "Importing combat data into template",
        "tiddlywiki_build" => "Building interactive report",
        "tiddlywiki_finalize" => "Finalizing report structure",
        "tiddlywiki_save" => "Saving final HTML report",
        
        // Legacy parser components
        "legacy_parser_start" => "Starting legacy report generation",
        "legacy_start" => "Starting legacy parser processing",
        "legacy_setup" => "Setting up legacy workspace",
        "legacy_moved_files" => "Processing log files for legacy parser",
        "legacy_tw5_done" => "Building legacy TiddlyWiki report",
        "legacy_cleanup" => "Finalizing legacy report",
        
        "cleanup" => "Cleaning up temporary files",
        "complete" => "Processing complete",
        
        _ => {
            // Fallback to progress-based messages
            if progress < 5.0 { "Initializing processing environment" }
            else if progress < 10.0 { "Verifying configuration files" }
            else if progress < 15.0 { "Starting Elite Insights analysis" }
            else if progress < 25.0 { "Processing logs with Elite Insights" }
            else if progress < 30.0 { "Starting TopStats analysis" }
            else if progress < 45.0 { "Analyzing player performance metrics" }
            else if progress < 55.0 { "Finalizing combat statistics" }
            else if progress < 60.0 { "Processing JSON combat data" }
            else if progress < 65.0 { "Starting report generation" }
            else if progress < 75.0 { "Building interactive report components" }
            else if progress < 85.0 { "Generating data visualizations" }
            else if progress < 95.0 { "Saving final report" }
            else if progress < 97.0 { "Cleaning temporary files" }
            else { "Almost done..." }
        }
    }.to_string()
}
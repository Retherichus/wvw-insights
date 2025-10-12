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
struct StatusResponse {
    status: String,
    progress: Option<f32>,
    logs: Option<Vec<LogEntry>>,
    files: Option<Vec<FileEntry>>,
    heartbeat: Option<Heartbeat>,
}

#[derive(Debug, Deserialize)]
struct LogEntry {
    message: String,
    #[serde(rename = "type")]
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


pub fn create_session(api_endpoint: &str, history_token: &str) -> Result<(String, String)> {
    let url = format!("{}?endpoint=nexus-session", api_endpoint);
    
    let response = CLIENT.with(|c| {
        c.post(&url)
            .send_form(&[
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
        // Use the multipart builder to create form data
        let builder = ureq_multipart::MultipartBuilder::new()
            .add_text("session_id", session_id)?
            .add_text("history_token", history_token)?
            .add_file("file", &location)?;
        
        let (content_type, data) = builder.finish()?;
        
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

pub fn start_processing(
    api_endpoint: &str,
    session_id: &str,
    history_token: &str,
    ownership_token: &str,
) -> Result<String> {
    let url = format!("{}?endpoint=nexus-process", api_endpoint);
    
    let response = CLIENT.with(|c| {
        c.post(&url)
            .send_form(&[
                ("session_id", session_id),
                ("history_token", history_token),
                ("ownership_token", ownership_token),
            ])
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

pub fn check_status(api_endpoint: &str, session_id: &str) -> Result<(String, Option<String>, f32, Option<String>)> {
    let url = format!("{}?endpoint=process-status&session_id={}", api_endpoint, session_id);
    
    let response = CLIENT.with(|c| c.get(&url).call())?;
    let status_resp: StatusResponse = response.into_json()?;
    
    log::info!("Status: {} - Progress: {:?}", status_resp.status, status_resp.progress);
    
    let progress = status_resp.progress.unwrap_or(0.0);
    
    // Get current phase from heartbeat component
    let phase = status_resp.heartbeat
        .and_then(|h| h.component)
        .map(|c| get_phase_message(&c, progress));
    
    let report_url = if status_resp.status == "complete" {
        status_resp.files
            .and_then(|files| {
                files.iter().find_map(|f| {
                    if f.name.contains("Report.html") {
                        Some(f.url.clone())
                    } else {
                        None
                    }
                })
            })
    } else {
        None
    };
    
    Ok((status_resp.status, report_url, progress, phase))
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
        "initialization" => "Initializing processing environment",
        "config_verification" => "Verifying configuration files",
        "elite_insights_start" => "Starting Elite Insights analysis",
        "elite_insights_executing" => "Running Elite Insights CLI",
        "elite_insights_processing" => "Processing log data",
        "elite_insights_complete" => "Elite Insights completed",
        "topstats_start" => "Starting TopStats analysis",
        "topstats_parsing" => "Parsing combat data",
        "topstats_processing" => "Analyzing performance metrics",
        "topstats_complete" => "Finalizing statistics",
        "json_processing" => "Processing JSON data",
        "tiddlywiki_start" => "Starting report generation",
        "tiddlywiki_build" => "Building interactive report",
        "tiddlywiki_finalize" => "Finalizing report",
        "cleanup" => "Cleaning up temporary files",
        "complete" => "Processing complete",
        _ => {
            // Fallback to progress-based messages
            if progress < 15.0 { "Initializing..." }
            else if progress < 25.0 { "Processing with Elite Insights..." }
            else if progress < 55.0 { "Running statistical analysis..." }
            else if progress < 85.0 { "Generating report..." }
            else if progress < 95.0 { "Finalizing..." }
            else { "Almost done..." }
        }
    }.to_string()
}

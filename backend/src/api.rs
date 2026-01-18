use axum::{
    extract::Json,
    response::IntoResponse,
    http::StatusCode,
};
use std::collections::HashMap;
use crate::models::{Message, MissionState, DashboardData, SystemStatus, DocInfo};
use crate::storage::{read_json, atomic_write_json, FileLock};
use axum::extract::Query;

use chrono::Local;

const LOG_FILE: &str = "conversation_log.json";
const STATE_FILE: &str = "mission_state.json";
const DASHBOARD_FILE: &str = "dashboard_data.json";

pub async fn get_messages() -> impl IntoResponse {
    match read_json::<Vec<Message>>(LOG_FILE) {
        Ok(messages) => (StatusCode::OK, Json(messages)).into_response(),
        Err(_) => (StatusCode::OK, Json(Vec::<Message>::new())).into_response(),
    }
}

pub async fn post_message(Json(mut new_msg): Json<Message>) -> impl IntoResponse {
    let _lock = match FileLock::new(LOG_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Lock failed").into_response(),
    };

    let mut messages: Vec<Message> = read_json(LOG_FILE).unwrap_or_default();
    new_msg.id = Some(messages.len());
    new_msg.timestamp = Some(Local::now().to_rfc3339());
    messages.push(new_msg.clone());

    if let Err(e) = atomic_write_json(LOG_FILE, &messages) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    (StatusCode::CREATED, Json(new_msg)).into_response()
}

// ... [previous message handlers]

pub async fn get_state() -> impl IntoResponse {
    match read_json::<MissionState>(STATE_FILE) {
        Ok(state) => (StatusCode::OK, Json(state)).into_response(),
        Err(_) => (StatusCode::OK, Json(serde_json::json!({}))).into_response(),
    }
}

pub async fn post_state(Json(updates): Json<serde_json::Value>) -> impl IntoResponse {
    let _lock = match FileLock::new(STATE_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Lock failed").into_response(),
    };

    let mut state: serde_json::Value = read_json(STATE_FILE).unwrap_or(serde_json::json!({}));
    
    // Simple merge logic
    if let (Some(state_obj), Some(updates_obj)) = (state.as_object_mut(), updates.as_object()) {
        for (k, v) in updates_obj {
            state_obj.insert(k.clone(), v.clone());
        }
    }

    if let Err(e) = atomic_write_json(STATE_FILE, &state) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    // --- Activity Logging ---
    let mut data: DashboardData = read_json(DASHBOARD_FILE).unwrap_or_default();
    let mut new_activities = vec![];
    let timestamp = Local::now().to_rfc3339();

    if let Some(updates_obj) = updates.as_object() {
        // Check for agent updates
        if let Some(agents) = updates_obj.get("agents").and_then(|a| a.as_object()) {
            for (agent_name, agent_data) in agents {
                if let Some(agent_obj) = agent_data.as_object() {
                    let status = agent_obj.get("status").and_then(|s| s.as_str());
                    let current_task = agent_obj.get("current_task").and_then(|s| s.as_str());
                    let last_task = agent_obj.get("last_task").and_then(|s| s.as_str());

                    let mut action_text = String::new();
                    let mut msg_type = "info";

                    if status == Some("busy") {
                        if let Some(task) = current_task {
                            let summary = if task.chars().count() > 50 { 
                                format!("{}...", task.chars().take(50).collect::<String>()) 
                            } else { 
                                task.to_string() 
                            };
                            action_text = format!("Started working on: {}", summary);
                        } else {
                            action_text = "Is now busy working on a task.".to_string();
                        }
                    } else if status == Some("idle") {
                        if let Some(task) = last_task {
                            let summary = if task.chars().count() > 50 { 
                                format!("{}...", task.chars().take(50).collect::<String>()) 
                            } else { 
                                task.to_string() 
                            };
                            action_text = format!("Completed task: {}", summary);
                            msg_type = "success";
                        } else {
                            action_text = "Completed task and is standing by.".to_string();
                            msg_type = "success";
                        }
                    } else if let Some(s) = status {
                        action_text = format!("Status changed to: {}", s);
                    }

                    if !action_text.is_empty() {
                        new_activities.push(crate::models::Activity {
                            time: timestamp.clone(),
                            agent: agent_name.clone(),
                            action: action_text,
                            activity_type: msg_type.to_string(),
                        });
                    }
                }
            }
        }

        // Check for mission status updates
        if let Some(status) = updates_obj.get("status").and_then(|s| s.as_str()) {
            new_activities.push(crate::models::Activity {
                time: timestamp,
                agent: "Mission Control".into(),
                action: format!("Mission status updated to: {}", status),
                activity_type: "warning".to_string(),
            });
        }
    }

    if !new_activities.is_empty() {
        let _lock = match FileLock::new(DASHBOARD_FILE) {
            Ok(l) => l,
            Err(_) => return (StatusCode::OK, Json(state)).into_response(), // Continue even if we can't lock dashboard
        };
        
        // Prepend new activities
        let mut all_activities = new_activities;
        all_activities.extend(data.activities);
        data.activities = all_activities.into_iter().take(50).collect();
        
        let _ = atomic_write_json(DASHBOARD_FILE, &data);
    }
    // -------------------------

    (StatusCode::OK, Json(state)).into_response()
}

pub async fn get_dashboard() -> impl IntoResponse {
    use std::process::Command;

    fn is_running(name: &str) -> bool {
        Command::new("pgrep")
            .arg("-f")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    let mut data: DashboardData = read_json(DASHBOARD_FILE).unwrap_or(DashboardData {
        global_status: "unknown".into(),
        systems: vec![],
        agents: HashMap::new(),
        activities: vec![],
        projects: vec![],
        metrics: vec![],
        docs: vec![],
        all_systems_go: false,
        disk: None,
    });

    if let Ok(state) = read_json::<MissionState>(STATE_FILE) {
        data.agents = state.agents;
    }

    let bridge_status = is_running("bridge") || is_running("browser_bridge.py");
    let sentry_status = is_running("sentry") || is_running("sentry.py");
    let server_status = true; // Rust server is running

    // --- Disk Info ---
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    if let Some(root) = disks.iter().find(|d| d.mount_point() == std::path::Path::new("/")) {
        let total = root.total_space();
        let available = root.available_space();
        let used = total - available;
        let usage_pct = (used as f64 / total as f64) * 100.0;
        
        // Calculate Workspace Size (async-ish walk)
        let workspace_size = crate::api::calculate_dir_size("/home/a2/Desktop/gem");

        data.disk = Some(crate::models::DiskStats {
            total,
            used,
            free: available,
            usage_pct,
            workspace_size,
        });
    }
    // -----------------

    data.systems = vec![
        SystemStatus { name: "🖥️ Server".into(), status: if server_status { "operational" } else { "down" }.into() },
        SystemStatus { name: "🌉 Browser Bridge".into(), status: if bridge_status { "operational" } else { "down" }.into() },
        SystemStatus { name: "🛡️ Sentry".into(), status: if sentry_status { "operational" } else { "down" }.into() },
    ];
    data.all_systems_go = bridge_status && sentry_status && server_status;
    data.global_status = if data.all_systems_go { "operational" } else { "critical" }.into();

    (StatusCode::OK, Json(data)).into_response()
}

// --- NEW Disk API ---

pub fn calculate_dir_size(path: &str) -> u64 {
    use walkdir::WalkDir;
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

pub async fn get_cleanup_candidates() -> impl IntoResponse {
    use walkdir::WalkDir;
    use crate::models::CleanupItem;
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut candidates = vec![];
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let seven_days_sec = 7 * 24 * 60 * 60;

    // 1. .gemini Cleanup
    let gemini_path = "/home/a2/.gemini/antigravity/brain";
    for entry in WalkDir::new(gemini_path).max_depth(1).into_iter().filter_map(|e| e.ok()) {
        if let Ok(metadata) = entry.metadata() {
            if metadata.is_file() || metadata.is_dir() {
                if let Ok(modified) = metadata.modified() {
                    let mod_sec = modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    if now - mod_sec > seven_days_sec {
                        candidates.push(CleanupItem {
                            id: format!("gemini_{}", entry.file_name().to_string_lossy()),
                            name: entry.file_name().to_string_lossy().into(),
                            path: entry.path().to_string_lossy().into(),
                            size: if metadata.is_file() { metadata.len() } else { calculate_dir_size(&entry.path().to_string_lossy()) },
                            category: "gemini".into(),
                        });
                    }
                }
            }
        }
    }

    // 2. System (Pacman) Cache check
    let pacman_cache = "/var/cache/pacman/pkg/";
    if std::path::Path::new(pacman_cache).exists() {
        let size = calculate_dir_size(pacman_cache);
        if size > 100 * 1024 * 1024 { // > 100MB
            candidates.push(CleanupItem {
                id: "system_pacman".into(),
                name: "Pacman Package Cache".into(),
                path: pacman_cache.into(),
                size,
                category: "system".into(),
            });
        }
    }

    // 3. System Journal check
    let journal_path = "/var/log/journal/";
    if std::path::Path::new(journal_path).exists() {
        let size = calculate_dir_size(journal_path);
        if size > 500 * 1024 * 1024 { // > 500MB
            candidates.push(CleanupItem {
                id: "system_journal".into(),
                name: "Systemd Journal Logs".into(),
                path: journal_path.into(),
                size,
                category: "system".into(),
            });
        }
    }

    (StatusCode::OK, Json(candidates)).into_response()
}

#[derive(serde::Deserialize)]
pub struct CleanupParams {
    path: String,
    category: String,
}

pub async fn post_cleanup(Json(params): Json<CleanupParams>) -> impl IntoResponse {
    use std::fs;
    use std::process::Command;

    println!("🧹 Cleanup requested for path: {}", params.path);

    let res = if params.category == "gemini" {
        if params.path.contains(".gemini") {
            let path = std::path::Path::new(&params.path);
            if path.is_file() {
                fs::remove_file(path)
            } else {
                fs::remove_dir_all(path)
            }
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Unsafe path"))
        }
    } else if params.category == "system" {
        // Limited to pacman cache and journal for now
        if params.path == "/var/cache/pacman/pkg/" {
            Command::new("sudo")
                .arg("pacman")
                .arg("-Sc")
                .arg("--noconfirm")
                .status()
                .map(|s| if s.success() { () } else { () })
                .map_err(|e| e)
        } else if params.path == "/var/log/journal/" {
            Command::new("sudo")
                .arg("journalctl")
                .arg("--vacuum-time=7d")
                .status()
                .map(|s| if s.success() { () } else { () })
                .map_err(|e| e)
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Unsafe system path"))
        }
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Unknown category"))
    };

    match res {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn post_dashboard(Json(data): Json<DashboardData>) -> impl IntoResponse {
    let _lock = match FileLock::new(DASHBOARD_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Lock failed").into_response(),
    };

    if let Err(e) = atomic_write_json(DASHBOARD_FILE, &data) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

pub async fn get_docs() -> impl IntoResponse {
    let data: DashboardData = read_json(DASHBOARD_FILE).unwrap_or(DashboardData {
        global_status: "unknown".into(),
        systems: vec![],
        activities: vec![],
        projects: vec![],
        metrics: vec![],
        docs: vec![],
        agents: HashMap::new(),
        all_systems_go: false,
        disk: None,
    });

    (StatusCode::OK, Json(serde_json::json!({ "docs": data.docs }))).into_response()
}

pub async fn post_docs(Json(doc_info): Json<DocInfo>) -> impl IntoResponse {
    let _lock = match FileLock::new(DASHBOARD_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Lock failed").into_response(),
    };

    let mut data: DashboardData = read_json(DASHBOARD_FILE).unwrap_or(DashboardData {
        global_status: "unknown".into(),
        systems: vec![],
        activities: vec![],
        projects: vec![],
        metrics: vec![],
        docs: vec![],
        agents: HashMap::new(),
        all_systems_go: false,
        disk: None,
    });

    // Check duplicates
    if !data.docs.iter().any(|d| d.path == doc_info.path) {
        data.docs.push(doc_info);
        if let Err(e) = atomic_write_json(DASHBOARD_FILE, &data) {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

#[derive(serde::Deserialize)]
pub struct ContentParams {
    path: String,
}

pub async fn get_docs_content(Query(params): Query<ContentParams>) -> impl IntoResponse {
    use std::fs;
    
    // SECURITY NOTE: This allows reading any file on the system.
    // In a prod environment, strictly limit paths. 
    // For this local tool, we assume user trusts themselves.
    
    match fs::read_to_string(&params.path) {
        Ok(content) => (StatusCode::OK, content).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, format!("Error reading file: {}", e)).into_response(),
    }
}

pub async fn delete_doc(Query(params): Query<ContentParams>) -> impl IntoResponse {
    let _lock = match FileLock::new(DASHBOARD_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Lock failed").into_response(),
    };

    let mut data: DashboardData = read_json(DASHBOARD_FILE).unwrap_or(DashboardData {
        global_status: "unknown".into(),
        systems: vec![],
        activities: vec![],
        projects: vec![],
        metrics: vec![],
        docs: vec![],
        agents: HashMap::new(),
        all_systems_go: false,
        disk: None,
    });

    let original_len = data.docs.len();
    data.docs.retain(|d| d.path != params.path);

    if data.docs.len() < original_len {
        if let Err(e) = atomic_write_json(DASHBOARD_FILE, &data) {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

pub async fn post_shutdown() -> impl IntoResponse {
    use std::process::Command;
    
    println!("🛑 Shutdown request received. Initiating graceful shutdown...");
    
    // Kill Bridge and Sentry processes
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("target/release/bridge")
        .output();
    
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("target/debug/bridge")
        .output();
    
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("target/release/sentry")
        .output();
    
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("target/debug/sentry")
        .output();
    
    println!("✅ Bridge and Sentry processes terminated.");
    
    // Spawn a thread to exit after sending response
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(500));
        println!("✅ Server shutting down now. Over.");
        std::process::exit(0);
    });
    
    (StatusCode::OK, Json(serde_json::json!({"status": "shutdown_initiated"}))).into_response()
}

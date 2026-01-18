use axum::{
    extract::Json,
    response::IntoResponse,
    http::StatusCode,
};
use crate::models::{Message, MissionState, DashboardData, SystemStatus, DocInfo};
use crate::storage::{read_json, atomic_write_json, FileLock, get_disk_usage};
use crate::system::{perform_cleanup, scan_cleanup_candidates};


use chrono::Local;

const LOG_FILE: &str = "conversation_log.json";
const STATE_FILE: &str = "mission_state.json";
const DASHBOARD_FILE: &str = "dashboard_data.json";
const PROJECTS_FILE: &str = "projects.json";
const DOCS_FILE: &str = "docs.json";

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

    // Activity logging removed for memory optimization

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

    let mut data: DashboardData = read_json(DASHBOARD_FILE).unwrap_or_default();
    data.projects = read_json::<Vec<crate::models::Project>>(PROJECTS_FILE).unwrap_or_default();
    data.docs = read_json::<Vec<DocInfo>>(DOCS_FILE).unwrap_or_default();
    data.activities = vec![]; // Logging is currently disabled

    if let Ok(state) = read_json::<MissionState>(STATE_FILE) {
        data.agents = state.agents;
    }

    let bridge_status = is_running("bridge") || is_running("browser_bridge.py");
    let sentry_status = is_running("sentry") || is_running("sentry.py");
    let server_status = true; // Rust server is running

    // --- Disk Info ---
    data.disk = get_disk_usage("/home/a2/Desktop/gem");
    // -----------------

    data.systems = vec![
        // --- Server (api.rs) ---
        SystemStatus { 
            name: "🖥️ Mission Control API".into(), 
            status: if server_status { "operational" } else { "down" }.into(),
            category: "Core Systems".into(),
            description: Some("Interface for frontend-backend communication".into())
        },
        SystemStatus { 
            name: "🗄️ Persistence Engine".into(), 
            status: if server_status { "operational" } else { "down" }.into(),
            category: "Core Systems".into(),
            description: Some("Data storage and log management".into())
        },
        SystemStatus { 
            name: "📄 Docs Engine".into(), 
            status: if server_status { "operational" } else { "down" }.into(),
            category: "Core Systems".into(),
            description: Some("Documentation registration and viewer services".into())
        },
        SystemStatus { 
            name: "💾 Disk Management".into(), 
            status: if server_status { "operational" } else { "down" }.into(),
            category: "Utilities".into(),
            description: Some("Resource analysis and cleanup tools".into())
        },
        // --- Browser Bridge (bridge.rs) ---
        SystemStatus { 
            name: "🌉 Browser Automation".into(), 
            status: if bridge_status { "operational" } else { "down" }.into(),
            category: "Automation".into(),
            description: Some("Chromium engine control and script execution".into())
        },
        SystemStatus { 
            name: "📡 Agent Relay".into(), 
            status: if bridge_status { "operational" } else { "down" }.into(),
            category: "Automation".into(),
            description: Some("Communication relay for AI agents".into())
        },
        // --- Sentry (sentry.rs) ---
        SystemStatus { 
            name: "🛡️ Workspace Auditor".into(), 
            status: if sentry_status { "operational" } else { "down" }.into(),
            category: "Monitoring".into(),
            description: Some("File integrity and change monitoring".into())
        },
        SystemStatus { 
            name: "🔭 System Watchtower".into(), 
            status: if sentry_status { "operational" } else { "down" }.into(),
            category: "Monitoring".into(),
            description: Some("Resource health and status monitoring".into())
        },
    ];
    data.all_systems_go = bridge_status && sentry_status && server_status;
    data.global_status = if data.all_systems_go { "operational" } else { "critical" }.into();

    (StatusCode::OK, Json(data)).into_response()
}

// --- NEW Disk API ---



pub async fn get_cleanup_candidates() -> impl IntoResponse {
    let candidates = scan_cleanup_candidates();
    (StatusCode::OK, Json(candidates)).into_response()
}

#[derive(serde::Deserialize)]
pub struct CleanupParams {
    path: String,
    category: String,
}

pub async fn post_cleanup(Json(params): Json<CleanupParams>) -> impl IntoResponse {
    println!("🧹 Cleanup requested for path: {}", params.path);

    let res = perform_cleanup(&params.path, &params.category);

    match res {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn post_dashboard(Json(data): Json<DashboardData>) -> impl IntoResponse {
    // Isolate and save projects
    let _proj_lock = match FileLock::new(PROJECTS_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Projects lock failed").into_response(),
    };
    if let Err(e) = atomic_write_json(PROJECTS_FILE, &data.projects) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    // Save other persistent dashboard meta if any
    let _dash_lock = match FileLock::new(DASHBOARD_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Dashboard lock failed").into_response(),
    };

    let mut persistent_data = data.clone();
    persistent_data.projects = vec![]; // Keep separate
    persistent_data.docs = vec![];     // Keep separate

    if let Err(e) = atomic_write_json(DASHBOARD_FILE, &persistent_data) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
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

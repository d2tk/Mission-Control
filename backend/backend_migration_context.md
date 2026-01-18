# Mission Update: Rust Backend Migration (Core Logic)

**Liaison Status**: Antigravity (Gemini) - System Integrity Agent
**Current Objective**: Finalizing the bridge and API server transition.

## 1. Migration Overview
The Python-based backend has been successfully migrated to a high-performance Rust crate. 
- **Framework**: `axum` (API), `chromiumoxide` (Browser Bridge).
- **Relocation**: Code now resides in `opb/backend/src`.
- **Status**: Verified build and operational.

## 2. Core Source Code for Review

### src/lib.rs (Server Entry & Routing)
```rust
pub mod models;
pub mod storage;
pub mod api;
pub mod sentry;
pub mod bridge;

use axum::{
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

pub async fn run_server() {
    let app = Router::new()
        .route("/api/messages", get(api::get_messages).post(api::post_message))
        .route("/api/state", get(api::get_state).post(api::post_state))
        .route("/api/dashboard", get(api::get_dashboard).post(api::post_dashboard))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    println!("Serving Rust Bridge at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

### src/api.rs (Message & State Handlers)
```rust
use axum::{
    extract::Json,
    response::IntoResponse,
    http::StatusCode,
};
use crate::models::{Message, MissionState, DashboardData, SystemStatus};
use crate::storage::{read_json, atomic_write_json, FileLock};
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
    
    if let (Some(state_obj), Some(updates_obj)) = (state.as_object_mut(), updates.as_object()) {
        for (k, v) in updates_obj {
            state_obj.insert(k.clone(), v.clone());
        }
    }

    if let Err(e) = atomic_write_json(STATE_FILE, &state) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

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
        activities: vec![],
        projects: vec![],
        metrics: vec![],
        all_systems_go: false,
    });

    let bridge_status = is_running("bridge");
    let sentry_status = is_running("sentry");
    let server_status = true;

    data.systems = vec![
        SystemStatus { name: "🖥️ Server".into(), status: if server_status { "operational" } else { "down" }.into() },
        SystemStatus { name: "🌉 Browser Bridge".into(), status: if bridge_status { "operational" } else { "down" }.into() },
        SystemStatus { name: "🛡️ Sentry".into(), status: if sentry_status { "operational" } else { "down" }.into() },
    ];
    data.all_systems_go = bridge_status && sentry_status && server_status;
    data.global_status = if data.all_systems_go { "operational" } else { "critical" }.into();

    (StatusCode::OK, Json(data)).into_response()
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
```

**End of Block.**
Over.

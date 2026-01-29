use axum::{
    extract::{Json, Query},
    response::IntoResponse,
    http::StatusCode,
};
use crate::models::DocInfo;
use crate::storage::{read_json, atomic_write_json, FileLock};

const DOCS_FILE: &str = "docs.json";

pub async fn get_docs() -> impl IntoResponse {
    let docs = read_json::<Vec<DocInfo>>(DOCS_FILE).unwrap_or_default();
    (StatusCode::OK, Json(serde_json::json!({ "docs": docs }))).into_response()
}

pub async fn fragment_logs() -> impl IntoResponse {
    use crate::models::Message;
    use chrono::Local;
    use std::fs;
    use std::path::Path;

    let log_file = "conversation_log.json";
    let messages: Vec<Message> = read_json(log_file).unwrap_or_default();
    
    if messages.is_empty() {
        return (StatusCode::OK, Json(serde_json::json!({"status": "no_logs"}))).into_response();
    }

    // Group by sender
    let mut gpt_logs = Vec::new();
    let mut claude_logs = Vec::new();
    let mut qwen_logs = Vec::new();

    for msg in &messages {
        match msg.sender.as_str() {
            "ChatGPT" => gpt_logs.push(msg.clone()),
            "Claude" => claude_logs.push(msg.clone()),
            "Ollama" | "Qwen" => qwen_logs.push(msg.clone()),
            _ => {}
        }
    }

    let date_str = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let fragments_dir = "backend/fragments";
    let _ = fs::create_dir_all(fragments_dir);

    let mut new_docs = Vec::new();

    let mut process_logs = |logs: Vec<Message>, agent: &str| {
        if logs.is_empty() { return; }
        
        let filename = format!("Log_{}_{}.json", agent, date_str);
        let path = format!("{}/{}", fragments_dir, filename);
        
        if let Ok(_) = atomic_write_json(&path, &logs) {
            new_docs.push(DocInfo {
                path: path.clone(),
                added: Local::now().to_rfc3339(),
                category: Some(format!("Log_{}", agent)),
                is_fragment: Some(true),
            });
        }
    };

    process_logs(gpt_logs, "GPT");
    process_logs(claude_logs, "Claude");
    process_logs(qwen_logs, "Qwen3");

    // Clear main log after fragmentation
    let _ = atomic_write_json(log_file, &Vec::<Message>::new());

    // Register in docs.json
    let _lock = match FileLock::new(DOCS_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Docs lock failed").into_response(),
    };
    let mut docs: Vec<DocInfo> = read_json(DOCS_FILE).unwrap_or_default();
    docs.extend(new_docs);
    let _ = atomic_write_json(DOCS_FILE, &docs);

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

pub async fn post_docs(Json(doc_info): Json<DocInfo>) -> impl IntoResponse {
    let _lock = match FileLock::new(DOCS_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Docs lock failed").into_response(),
    };

    let mut docs: Vec<DocInfo> = read_json(DOCS_FILE).unwrap_or_default();

    // Check duplicates
    if !docs.iter().any(|d| d.path == doc_info.path) {
        docs.push(doc_info);
        if let Err(e) = atomic_write_json(DOCS_FILE, &docs) {
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
    let _lock = match FileLock::new(DOCS_FILE) {
        Ok(l) => l,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Docs lock failed").into_response(),
    };

    let mut docs: Vec<DocInfo> = read_json(DOCS_FILE).unwrap_or_default();
    
    // Check if it's a fragment and delete physical file
    if let Some(doc) = docs.iter().find(|d| d.path == params.path) {
        if doc.is_fragment.unwrap_or(false) {
            let _ = std::fs::remove_file(&params.path);
        }
    }

    let original_len = docs.len();
    docs.retain(|d| d.path != params.path);

    if docs.len() < original_len {
        if let Err(e) = atomic_write_json(DOCS_FILE, &docs) {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

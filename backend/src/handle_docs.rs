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
    let original_len = docs.len();
    docs.retain(|d| d.path != params.path);

    if docs.len() < original_len {
        if let Err(e) = atomic_write_json(DOCS_FILE, &docs) {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

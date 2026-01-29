pub mod models;
pub mod storage;
pub mod api;
pub mod sentry;
pub mod automation;
pub mod system;
pub mod handle_docs;


use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

pub async fn run_server() {
    let app = Router::new()
        .route("/api/messages", get(api::get_messages).post(api::post_message))
        .route("/api/state", get(api::get_state).post(api::post_state))
        .route("/api/dashboard", get(api::get_dashboard).post(api::post_dashboard))
        .route("/api/docs", get(handle_docs::get_docs).post(handle_docs::post_docs).delete(handle_docs::delete_doc))
        .route("/api/docs/content", get(handle_docs::get_docs_content))
        .route("/api/disk/cleanup", get(api::get_cleanup_candidates).post(api::post_cleanup))
        .route("/api/shutdown", post(api::post_shutdown))
        .route("/api/logs/fragment", post(handle_docs::fragment_logs))
        .route("/api/podman", post(api::post_podman))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("❌ Fatal Error: Could not bind to port 8000: {}", e);
            eprintln!("   Ensure no other server instance is running or wait for the OS to release the port.");
            std::process::exit(1);
        }
    };

    println!("Serving Mission Control Server at http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}

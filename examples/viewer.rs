//! HLS Viewer Example
//!
//! A minimal web server that serves the HLS viewer interface.
//! Run with: `cargo run --example viewer`

use axum::{response::Html, routing::get, Router};
use std::env;

const VIEWER_HTML: &str = include_str!("viewer/index.html");

#[tokio::main]
async fn main() {
    let port = env::var("VIEWER_PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("127.0.0.1:{}", port);

    println!("🎬 Sinatra HLS Viewer");
    println!("   Viewer:  http://{}", addr);
    println!("   API:     http://127.0.0.1:3000");
    println!();
    println!("Make sure the main server is running:");
    println!("   UPLOAD_DIR=./test_vars WAITING_DIR=./test_vars/waiting cargo run");
    println!();

    let app = Router::new().route("/", get(serve_viewer));

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind viewer");

    println!("Viewer ready at http://{}", addr);

    axum::serve(listener, app)
        .await
        .expect("Viewer server failed");
}

async fn serve_viewer() -> Html<&'static str> {
    Html(VIEWER_HTML)
}

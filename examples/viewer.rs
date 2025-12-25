//! HLS Viewer Example
//!
//! A minimal web server that serves the HLS viewer interface.
//! Run with: `cargo run --example viewer`

use axum::{
    extract::{Query, State},
    response::{Html, Json},
    routing::get,
    Router,
};
use std::env;
use std::time::Duration;

use aws_config::BehaviorVersion;
use aws_sdk_s3::{
    config::{Credentials, Region},
    presigning::PresigningConfig,
    Client,
};
use serde::{Deserialize, Serialize};

const VIEWER_HTML: &str = include_str!("viewer/index.html");

#[derive(Clone)]
struct AppState {
    client: Client,
}

#[tokio::main]
async fn main() {
    let port = env::var("VIEWER_PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("127.0.0.1:{}", port);
    let s3_endpoint =
        env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());

    println!("🎬 Sinatra HLS Viewer");
    println!("   Viewer:  http://{}", addr);
    println!("   API:     {}", s3_endpoint);
    println!();

    // Configure S3 Client
    let access_key = env::var("AWS_ACCESS_KEY_ID").unwrap_or("minioadmin".to_string());
    let secret_key = env::var("AWS_SECRET_ACCESS_KEY").unwrap_or("minioadmin".to_string());

    let credentials = Credentials::new(access_key, secret_key, None, None, "static");
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .credentials_provider(credentials)
        .endpoint_url(&s3_endpoint)
        .load()
        .await;

    // We need to force path style for local minio/s3s usually, or ensure DNS works.
    // aws-sdk-s3 defaults to virtual hosted style (bucket.domain) unless configured.
    let s3_config = aws_sdk_s3::config::Builder::from(&config)
        .force_path_style(true)
        .build();

    let client = Client::from_conf(s3_config);
    let state = AppState { client };

    let app = Router::new()
        .route("/", get(serve_viewer))
        .route("/presign", get(get_presigned_url))
        .with_state(state);

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

#[derive(Deserialize)]
struct PresignRequest {
    filename: String,
    file_type: String,
    size: u64,
}

#[derive(Serialize)]
struct PresignResponse {
    url: String,
    uuid_key: String,
}

async fn get_presigned_url(
    State(state): State<AppState>,
    Query(params): Query<PresignRequest>,
) -> Result<Json<PresignResponse>, (axum::http::StatusCode, String)> {
    // Validation
    const MAX_SIZE: u64 = 10 * 1024 * 1024; // 10MB

    if params.size > MAX_SIZE {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            format!(
                "File too large. Maximum size is {}MB",
                MAX_SIZE / 1024 / 1024
            ),
        ));
    }

    if !params.file_type.starts_with("video/") {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid file type. Only video files are allowed.".to_string(),
        ));
    }

    let uuid = uuid::Uuid::new_v4().to_string();
    // Keep the extension if present
    let extension = std::path::Path::new(&params.filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    let key = if extension.is_empty() {
        uuid.clone()
    } else {
        format!("{}.{}", uuid, extension)
    };

    let expires_in = Duration::from_secs(3600); // 1 hour
                                                // Note: We intentionally do NOT include .metadata() here because:
                                                // 1. The browser must send the EXACT same header values
                                                // 2. URL encoding/character escaping can cause mismatches
                                                // 3. x-amz-meta headers are signed, so any difference = SignatureDoesNotMatch
    let presigned_request = state
        .client
        .put_object()
        .bucket("stream")
        .key(&key)
        .content_type(&params.file_type)
        .presigned(PresigningConfig::expires_in(expires_in).unwrap())
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to sign request: {}", e),
            )
        })?;

    Ok(Json(PresignResponse {
        url: presigned_request.uri().to_string(),
        uuid_key: key,
    }))
}

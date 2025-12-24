//! Monolith Binary - Local deployment with S3-compatible API
//!
//! This is the main entry point for local development and single-server deployment.
//! It wires up:
//! - Local adapters (filesystem, Redis)
//! - HTTP/S3-compatible inbound adapter
//! - Event-driven video processing pipeline

use axum::{extract::DefaultBodyLimit, Router};
use sinatra::adapters::local::{buckets, events, fs::FsAdapter, redis::RedisQueue, LocalS3};
use sinatra::application::{orchestrator::OrchestratorService, worker::WorkerService};
use sinatra::config::LocalConfig;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    let config = LocalConfig::from_env();

    tracing_subscriber::fmt::init();

    // 1. Adapters (Local implementations)
    let redis_queue = match RedisQueue::new(&config.redis_url) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("Failed to connect to Redis: {:?}", e);
            std::process::exit(1);
        }
    };

    let fs_adapter = FsAdapter::new();

    // 2. Application Services
    let orchestrator = Arc::new(OrchestratorService::new(
        fs_adapter,
        redis_queue.clone(),
        redis_queue.clone(),
    ));

    let worker_service = Arc::new(WorkerService::new(
        fs_adapter,
        redis_queue.clone(),
        redis_queue.clone(),
    ));

    // 3. Start Workers
    let num_workers = 15;
    for i in 0..num_workers {
        let w = worker_service.clone();
        tokio::spawn(async move {
            w.run_worker_loop(i).await;
        });
    }
    println!("Started {} transcoding workers", num_workers);

    // 4. Event System (Local only - for S3 upload notifications)
    let event_hub = Arc::new(events::hub::EventHub::new());
    events::listener::start(event_hub.clone(), orchestrator.clone());

    // 5. HTTP Layer (S3-compatible API - Local only)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new().layer(cors).layer(DefaultBodyLimit::disable());

    let s3_impl = LocalS3 {
        event_hub: event_hub.clone(),
        upload_dir: PathBuf::from(&config.upload_dir),
    };

    let s3_service = buckets::auth::create_service(&config, s3_impl);
    let app = router.fallback_service(s3_service);

    // 6. Start Server
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.addr, config.port))
        .await
        .expect("Failed to bind TCP listener");
    println!("Listening at {}:{}", config.addr, config.port);
    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}

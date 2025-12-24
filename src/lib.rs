//! Sinatra - Video Processing Library
//!
//! Hexagonal Architecture:
//! - domain/: Pure business logic (av, hls, jobs)
//! - ports/: Trait definitions
//! - adapters/: Concrete implementations
//! - application/: Generic services
//! - config: Environment configuration
//!
//! # Features
//! - `local`: Local/monolith deployment (Redis, HTTP/S3-compatible API, video processing)
//! - `aws_orchestrator`: AWS orchestrator (S3, SQS, DynamoDB, video analysis)
//! - `aws_worker`: AWS worker (S3, SQS, DynamoDB, video processing)
//! - `aws`: Both aws_orchestrator and aws_worker
//! - `full`: All features

pub mod adapters;
pub mod application;
pub mod config;
pub mod domain;
pub mod ports;

// Re-exports for convenience
#[cfg(feature = "local")]
pub use adapters::local::{buckets, events, LocalS3};

#[cfg(any(feature = "aws_orchestrator", feature = "aws_worker"))]
pub use config::AwsConfig;

#[cfg(feature = "local")]
pub use config::LocalConfig;

// av module available for local, orchestrator, and worker
#[cfg(any(
    feature = "local",
    feature = "aws_orchestrator",
    feature = "aws_worker"
))]
pub use domain::av;

// hls module only for local and worker (not orchestrator)
#[cfg(any(feature = "local", feature = "aws_worker"))]
pub use domain::hls;

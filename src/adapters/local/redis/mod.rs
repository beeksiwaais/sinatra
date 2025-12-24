//! Redis adapter for local deployment.
//!
//! This module provides Redis-backed implementations of:
//! - `JobQueuePort` for job enqueueing/dequeueing
//! - `VideoStateRepository` for video status tracking

mod error;
mod pool;
mod queue;
mod repository;

pub use error::QueueError;
pub use pool::RedisPool;

// Backwards compatibility alias
pub type RedisQueue = RedisPool;

/// Redis key constants
const SEGMENT_QUEUE_HIGH_PRIORITY: &str = "sinatra:segment_jobs:high";
const SEGMENT_QUEUE_NORMAL: &str = "sinatra:segment_jobs:normal";
const VIDEO_STATUS_PREFIX: &str = "sinatra:video:";
const VIDEO_COMPLETED_PREFIX: &str = "sinatra:video_completed:";

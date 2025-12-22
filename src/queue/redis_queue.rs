use crate::queue::job::{SegmentJob, VideoStatus};
use deadpool_redis::redis::AsyncCommands;
use deadpool_redis::{Config, CreatePoolError, Pool, Runtime};
use std::fmt;

/// High priority queue for first 2 segments (enables faster playback start)
const SEGMENT_QUEUE_HIGH_PRIORITY: &str = "sinatra:segment_jobs:high";
/// Normal priority queue for remaining segments
const SEGMENT_QUEUE_NORMAL: &str = "sinatra:segment_jobs:normal";
const VIDEO_STATUS_PREFIX: &str = "sinatra:video:";
const VIDEO_COMPLETED_PREFIX: &str = "sinatra:video_completed:";

pub type RedisError = deadpool_redis::redis::RedisError;
pub type PoolError = deadpool_redis::PoolError;

#[derive(Debug)]
pub enum QueueError {
    Redis(RedisError),
    Pool(PoolError),
    Serialization(serde_json::Error),
    CreatePool(String),
}

impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueueError::Redis(e) => write!(f, "Redis error: {}", e),
            QueueError::Pool(e) => write!(f, "Pool error: {}", e),
            QueueError::Serialization(e) => write!(f, "Serialization error: {}", e),
            QueueError::CreatePool(e) => write!(f, "Create pool error: {}", e),
        }
    }
}

impl std::error::Error for QueueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            QueueError::Redis(e) => Some(e),
            QueueError::Pool(e) => Some(e),
            QueueError::Serialization(e) => Some(e),
            QueueError::CreatePool(_) => None,
        }
    }
}

impl From<RedisError> for QueueError {
    fn from(err: RedisError) -> Self {
        QueueError::Redis(err)
    }
}

impl From<PoolError> for QueueError {
    fn from(err: PoolError) -> Self {
        QueueError::Pool(err)
    }
}

impl From<serde_json::Error> for QueueError {
    fn from(err: serde_json::Error) -> Self {
        QueueError::Serialization(err)
    }
}

impl From<CreatePoolError> for QueueError {
    fn from(err: CreatePoolError) -> Self {
        QueueError::CreatePool(format!("{}", err))
    }
}

/// Redis-backed queue for segment transcoding jobs
pub struct RedisQueue {
    pool: Pool,
}

impl RedisQueue {
    /// Create a new RedisQueue with connection pool
    pub fn new(redis_url: &str) -> Result<Self, QueueError> {
        let cfg = Config::from_url(redis_url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        Ok(Self { pool })
    }

    /// Store video metadata in Redis
    pub async fn set_video_status(&self, status: &VideoStatus) -> Result<(), QueueError> {
        let mut conn = self.pool.get().await?;
        let key = format!("{}{}", VIDEO_STATUS_PREFIX, status.id);
        let json = serde_json::to_string(status)?;
        conn.set::<_, _, ()>(&key, json).await?;
        // Initialize completed counter to 0
        let completed_key = format!("{}{}", VIDEO_COMPLETED_PREFIX, status.id);
        conn.set::<_, _, ()>(&completed_key, 0i64).await?;
        Ok(())
    }

    /// Get video status from Redis
    pub async fn get_video_status(
        &self,
        video_id: &str,
    ) -> Result<Option<VideoStatus>, QueueError> {
        let mut conn = self.pool.get().await?;
        let key = format!("{}{}", VIDEO_STATUS_PREFIX, video_id);
        let json: Option<String> = conn.get(&key).await?;
        match json {
            Some(data) => Ok(Some(serde_json::from_str(&data)?)),
            None => Ok(None),
        }
    }

    /// Push a segment job to the appropriate priority queue
    /// Segments 0 and 1 go to high priority for faster playback start
    pub async fn enqueue_segment(&self, job: &SegmentJob) -> Result<(), QueueError> {
        let mut conn = self.pool.get().await?;
        let json = serde_json::to_string(job)?;

        // First 2 segments get high priority for faster playback start
        let queue_key = if job.segment_index < 2 {
            SEGMENT_QUEUE_HIGH_PRIORITY
        } else {
            SEGMENT_QUEUE_NORMAL
        };

        conn.lpush::<_, _, ()>(queue_key, json).await?;
        Ok(())
    }

    /// Pop a segment job from queues, checking high priority first
    /// Uses non-blocking RPOP on high priority, then BRPOP on normal if empty
    pub async fn dequeue_segment(
        &self,
        timeout_secs: f64,
    ) -> Result<Option<SegmentJob>, QueueError> {
        let mut conn = self.pool.get().await?;

        // First, try high priority queue (non-blocking)
        let high_result: Option<String> = conn.rpop(SEGMENT_QUEUE_HIGH_PRIORITY, None).await?;
        if let Some(json) = high_result {
            return Ok(Some(serde_json::from_str(&json)?));
        }

        // No high priority jobs, block on normal queue
        let result: Option<(String, String)> =
            conn.brpop(SEGMENT_QUEUE_NORMAL, timeout_secs).await?;
        match result {
            Some((_, json)) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    /// Atomically increment completed segment count and return new value
    pub async fn mark_segment_complete(&self, video_id: &str) -> Result<u64, QueueError> {
        let mut conn = self.pool.get().await?;
        let key = format!("{}{}", VIDEO_COMPLETED_PREFIX, video_id);
        let count: u64 = conn.incr(&key, 1i64).await?;
        Ok(count)
    }

    /// Get total segments for a video
    pub async fn get_total_segments(&self, video_id: &str) -> Result<u64, QueueError> {
        match self.get_video_status(video_id).await? {
            Some(status) => Ok(status.total_segments as u64),
            None => Ok(0),
        }
    }

    /// Clean up video data from Redis after processing
    pub async fn cleanup_video(&self, video_id: &str) -> Result<(), QueueError> {
        let mut conn = self.pool.get().await?;
        let status_key = format!("{}{}", VIDEO_STATUS_PREFIX, video_id);
        let completed_key = format!("{}{}", VIDEO_COMPLETED_PREFIX, video_id);
        conn.del::<_, ()>(&[status_key, completed_key]).await?;
        Ok(())
    }
}

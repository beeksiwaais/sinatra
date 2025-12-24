//! Redis JobQueuePort implementation.

use super::error::QueueError;
use super::pool::RedisPool;
use super::{SEGMENT_QUEUE_HIGH_PRIORITY, SEGMENT_QUEUE_NORMAL};
use crate::domain::jobs::Job;
use crate::ports::queue::JobQueuePort;
use async_trait::async_trait;
use deadpool_redis::redis::AsyncCommands;

#[async_trait]
impl JobQueuePort for RedisPool {
    async fn enqueue_job(&self, job: Job) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.pool.get().await.map_err(QueueError::from)?;
        let json = serde_json::to_string(&job)?;

        let is_high_priority = match &job {
            Job::Segment(seg) => seg.segment_index < 2,
            Job::ThumbnailStrip(_) => false,
        };

        let queue_key = if is_high_priority {
            SEGMENT_QUEUE_HIGH_PRIORITY
        } else {
            SEGMENT_QUEUE_NORMAL
        };

        conn.lpush::<_, _, ()>(queue_key, json)
            .await
            .map_err(QueueError::from)?;
        Ok(())
    }

    async fn dequeue_job(
        &self,
        timeout_secs: f64,
    ) -> Result<Option<Job>, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.pool.get().await.map_err(QueueError::from)?;

        // First, try high priority queue (non-blocking)
        let high_result: Option<String> = conn
            .rpop(SEGMENT_QUEUE_HIGH_PRIORITY, None)
            .await
            .map_err(QueueError::from)?;
        if let Some(json) = high_result {
            return Ok(Some(serde_json::from_str(&json)?));
        }

        // No high priority jobs, block on normal queue
        let result: Option<(String, String)> = conn
            .brpop(SEGMENT_QUEUE_NORMAL, timeout_secs)
            .await
            .map_err(QueueError::from)?;
        match result {
            Some((_, json)) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }
}

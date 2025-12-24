//! Redis VideoStateRepository implementation.

use super::error::QueueError;
use super::pool::RedisPool;
use super::{VIDEO_COMPLETED_PREFIX, VIDEO_STATUS_PREFIX};
use crate::domain::jobs::VideoStatus;
use crate::ports::repository::VideoStateRepository;
use async_trait::async_trait;
use deadpool_redis::redis::AsyncCommands;

#[async_trait]
impl VideoStateRepository for RedisPool {
    async fn save_video_status(
        &self,
        status: &VideoStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.pool.get().await.map_err(QueueError::from)?;
        let key = format!("{}{}", VIDEO_STATUS_PREFIX, status.id);
        let json = serde_json::to_string(status)?;
        conn.set::<_, _, ()>(&key, json)
            .await
            .map_err(QueueError::from)?;
        let completed_key = format!("{}{}", VIDEO_COMPLETED_PREFIX, status.id);
        conn.set::<_, _, ()>(&completed_key, 0i64)
            .await
            .map_err(QueueError::from)?;
        Ok(())
    }

    async fn get_video_status(
        &self,
        video_id: &str,
    ) -> Result<Option<VideoStatus>, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.pool.get().await.map_err(QueueError::from)?;
        let key = format!("{}{}", VIDEO_STATUS_PREFIX, video_id);
        let json: Option<String> = conn.get(&key).await.map_err(QueueError::from)?;
        match json {
            Some(data) => Ok(Some(serde_json::from_str(&data)?)),
            None => Ok(None),
        }
    }

    async fn mark_segment_complete(
        &self,
        video_id: &str,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.pool.get().await.map_err(QueueError::from)?;
        let key = format!("{}{}", VIDEO_COMPLETED_PREFIX, video_id);
        let count: u64 = conn.incr(&key, 1i64).await.map_err(QueueError::from)?;
        Ok(count as usize)
    }

    async fn get_total_segments(
        &self,
        video_id: &str,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.pool.get().await.map_err(QueueError::from)?;
        let key = format!("{}{}", VIDEO_STATUS_PREFIX, video_id);
        let json: Option<String> = conn.get(&key).await.map_err(QueueError::from)?;
        match json {
            Some(data) => {
                let status: VideoStatus = serde_json::from_str(&data)?;
                Ok(status.total_segments)
            }
            None => Ok(0),
        }
    }

    async fn cleanup_video(
        &self,
        video_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.pool.get().await.map_err(QueueError::from)?;
        let status_key = format!("{}{}", VIDEO_STATUS_PREFIX, video_id);
        let completed_key = format!("{}{}", VIDEO_COMPLETED_PREFIX, video_id);
        conn.del::<_, ()>(&[status_key, completed_key])
            .await
            .map_err(QueueError::from)?;
        Ok(())
    }
}

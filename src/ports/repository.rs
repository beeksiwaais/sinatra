use crate::domain::jobs::VideoStatus;
use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait VideoStateRepository: Send + Sync {
    /// Initialize video status
    async fn save_video_status(
        &self,
        status: &VideoStatus,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Get video status
    async fn get_video_status(
        &self,
        video_id: &str,
    ) -> Result<Option<VideoStatus>, Box<dyn Error + Send + Sync>>;

    /// Mark a segment as complete
    /// Returns current completion count
    async fn mark_segment_complete(
        &self,
        video_id: &str,
    ) -> Result<usize, Box<dyn Error + Send + Sync>>;

    /// Get total segments for a video
    async fn get_total_segments(
        &self,
        video_id: &str,
    ) -> Result<usize, Box<dyn Error + Send + Sync>>;

    /// Cleanup video state (after completion)
    async fn cleanup_video(&self, video_id: &str) -> Result<(), Box<dyn Error + Send + Sync>>;
}

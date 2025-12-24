use crate::domain::jobs::Job;
use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait JobQueuePort: Send + Sync {
    /// Enqueue a job
    async fn enqueue_job(&self, job: Job) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Dequeue a job (blocking with timeout or non-blocking)
    /// timeout_secs: 0.0 for infinite (or long poll), >0.0 for specific timeout
    async fn dequeue_job(
        &self,
        timeout_secs: f64,
    ) -> Result<Option<Job>, Box<dyn Error + Send + Sync>>;
}

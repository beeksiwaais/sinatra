use super::super::events::hub::EventHub;
use s3s::dto::*;
use s3s::S3;
use s3s::{S3Request, S3Response, S3Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Local S3-compatible service for monolith deployment.
#[derive(Debug, Clone)]
pub struct LocalS3 {
    pub event_hub: Arc<EventHub>,
    /// Base directory for file storage
    pub upload_dir: PathBuf,
}

#[async_trait::async_trait]
impl S3 for LocalS3 {
    async fn put_object(
        &self,
        req: S3Request<PutObjectInput>,
    ) -> S3Result<S3Response<PutObjectOutput>> {
        super::buckets::put::handle(self, req).await
    }

    async fn get_object(
        &self,
        req: S3Request<GetObjectInput>,
    ) -> S3Result<S3Response<GetObjectOutput>> {
        super::buckets::get::handle(self, req).await
    }

    async fn head_object(
        &self,
        req: S3Request<HeadObjectInput>,
    ) -> S3Result<S3Response<HeadObjectOutput>> {
        super::buckets::head::handle(self, req).await
    }
}

use crate::ports::storage::StoragePort;
use async_trait::async_trait;
use aws_sdk_s3::Client;
use std::error::Error;
use std::path::Path;

/// S3Adapter implements StoragePort for AWS S3.
#[derive(Clone)]
pub struct S3Adapter {
    client: Client,
    bucket: String,
}

impl S3Adapter {
    pub fn new(client: Client, bucket: String) -> Self {
        Self { client, bucket }
    }
}

#[async_trait]
impl StoragePort for S3Adapter {
    async fn download(
        &self,
        key: &str,
        local_path: &Path,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;

        let body = resp.body.collect().await?;
        tokio::fs::write(local_path, body.into_bytes()).await?;
        Ok(())
    }

    async fn upload(
        &self,
        local_path: &Path,
        key: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let body = tokio::fs::read(local_path).await?;
        let byte_stream = aws_sdk_s3::primitives::ByteStream::from(body);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(byte_stream)
            .send()
            .await?;
        Ok(())
    }
}

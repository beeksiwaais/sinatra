use async_trait::async_trait;
use std::error::Error;
use std::path::Path;

#[async_trait]
pub trait StoragePort: Send + Sync {
    /// Download a file from storage to a local path
    async fn download(
        &self,
        key: &str,
        local_path: &Path,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Upload a file from a local path to storage
    async fn upload(
        &self,
        local_path: &Path,
        key: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

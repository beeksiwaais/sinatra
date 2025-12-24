use crate::ports::storage::StoragePort;
use async_trait::async_trait;
use std::error::Error;
use std::path::Path;

#[derive(Clone, Copy)]
pub struct FsAdapter;

impl FsAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StoragePort for FsAdapter {
    async fn download(
        &self,
        key: &str,
        local_path: &Path,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // In local mode, "key" is assumed to be a path (or we copy from "source storage" to "temp").
        // But in monolith, we just use the file in-place usually.
        // However, to strictly follow the port:
        // "download" means ensuring the file exists at local_path.
        // If key == local_path (string representation), we do nothing.
        // If they differ, we copy.

        let key_path = Path::new(key);
        if key_path != local_path {
            if let Some(parent) = local_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::copy(key_path, local_path).await?;
        }
        Ok(())
    }

    async fn upload(
        &self,
        local_path: &Path,
        key: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Copy from local_path to key (destination)
        let key_path = Path::new(key);
        if key_path != local_path {
            if let Some(parent) = key_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::copy(local_path, key_path).await?;
        }
        Ok(())
    }
}

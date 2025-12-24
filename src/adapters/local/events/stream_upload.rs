use crate::application::orchestrator::OrchestratorService;
use crate::ports::{queue::JobQueuePort, repository::VideoStateRepository, storage::StoragePort};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub async fn handle<S, Q, R>(
    path: PathBuf,
    metadata: Option<HashMap<String, String>>,
    orchestrator: Arc<OrchestratorService<S, Q, R>>,
) where
    S: StoragePort,
    Q: JobQueuePort,
    R: VideoStateRepository,
{
    println!("Event: StreamUpload for {:?}", path);
    if let Some(meta) = metadata {
        println!("  Metadata: {:?}", meta);
    }

    // Convert path to key. For local FS, key is the path string.
    let key = path.to_string_lossy().to_string();

    if let Err(e) = orchestrator.handle_new_video(&key).await {
        eprintln!("Error enqueuing: {:?}", e);
    }
}

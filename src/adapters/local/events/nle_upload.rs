use crate::application::orchestrator::OrchestratorService;
use crate::ports::{queue::JobQueuePort, repository::VideoStateRepository, storage::StoragePort};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub async fn handle<S, Q, R>(
    _path: PathBuf,
    metadata: Option<HashMap<String, String>>,
    _orchestrator: Arc<OrchestratorService<S, Q, R>>,
) where
    S: StoragePort,
    Q: JobQueuePort,
    R: VideoStateRepository,
{
    println!("Event: NleUpload - Not Implemented (Orchestrator ignored)");
    if let Some(meta) = metadata {
        println!("  Metadata: {:?}", meta);
    }
}

use super::hub::EventHub;
use super::FileEvent;
use crate::application::orchestrator::OrchestratorService;
use crate::ports::{queue::JobQueuePort, repository::VideoStateRepository, storage::StoragePort};
use std::sync::Arc;

pub fn start<S, Q, R>(event_hub: Arc<EventHub>, orchestrator: Arc<OrchestratorService<S, Q, R>>)
where
    S: StoragePort + Send + Sync + 'static,
    Q: JobQueuePort + Send + Sync + 'static,
    R: VideoStateRepository + Send + Sync + 'static,
{
    let mut rx = event_hub.subscribe();

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                FileEvent::StreamUpload {
                    path,
                    bucket: _,
                    metadata,
                } => {
                    super::stream_upload::handle(path, metadata, orchestrator.clone()).await;
                }
                FileEvent::NleUpload {
                    path,
                    bucket: _,
                    metadata,
                } => {
                    super::nle_upload::handle(path, metadata, orchestrator.clone()).await;
                }
            }
        }
    });
}

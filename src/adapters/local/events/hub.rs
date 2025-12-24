use super::FileEvent;
use tokio::sync::broadcast;

#[derive(Debug)]
pub struct EventHub {
    sender: broadcast::Sender<FileEvent>,
}

impl EventHub {
    pub fn new() -> Self {
        // Capacity of 100 events should be sufficient for now
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    pub fn publish(
        &self,
        event: FileEvent,
    ) -> Result<usize, broadcast::error::SendError<FileEvent>> {
        self.sender.send(event)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<FileEvent> {
        self.sender.subscribe()
    }
}

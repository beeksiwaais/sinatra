use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub mod hub;
pub mod listener;
pub mod nle_upload;
pub mod stream_upload;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileEvent {
    StreamUpload {
        path: PathBuf,
        bucket: String,
        metadata: Option<HashMap<String, String>>,
    },
    NleUpload {
        path: PathBuf,
        bucket: String,
        metadata: Option<HashMap<String, String>>,
    },
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailStripJob {
    pub id: String,
    pub video_id: String,
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub interval_seconds: u32,
    pub width: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Job {
    Segment(SegmentJob),
    ThumbnailStrip(ThumbnailStripJob),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentJob {
    pub id: String,
    pub video_id: String,
    pub segment_index: usize,
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub start_time: f64,
    pub duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStatus {
    pub id: String,
    pub source_path: PathBuf,
    pub hls_dir: PathBuf,
    pub total_segments: usize,
    pub segment_durations: Vec<f64>,
}

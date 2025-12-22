use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a single segment transcoding job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentJob {
    /// Unique job ID
    pub id: String,
    /// Parent video ID - used to track completion
    pub video_id: String,
    /// Segment index (0-based)
    pub segment_index: usize,
    /// Path to source video file
    pub source_path: PathBuf,
    /// Output path for transcoded segment
    pub output_path: PathBuf,
    /// Start time in seconds
    pub start_time: f64,
    /// Duration in seconds
    pub duration: f64,
}

/// Metadata for a video being processed, stored in Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStatus {
    /// Unique video ID
    pub id: String,
    /// Original source path
    pub source_path: PathBuf,
    /// HLS output directory
    pub hls_dir: PathBuf,
    /// Total number of segments
    pub total_segments: usize,
    /// Segment durations (for playlist generation)
    pub segment_durations: Vec<f64>,
}

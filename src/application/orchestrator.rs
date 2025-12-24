use crate::domain::av::av::AV;
use crate::domain::jobs::{Job, SegmentJob, ThumbnailStripJob, VideoStatus};
use crate::ports::queue::JobQueuePort;
use crate::ports::repository::VideoStateRepository;
use crate::ports::storage::StoragePort;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use uuid::Uuid;

pub struct OrchestratorService<S, Q, R> {
    storage: S,
    queue: Q,
    repo: R,
}

impl<S, Q, R> OrchestratorService<S, Q, R>
where
    S: StoragePort,
    Q: JobQueuePort,
    R: VideoStateRepository,
{
    pub fn new(storage: S, queue: Q, repo: R) -> Self {
        Self {
            storage,
            queue,
            repo,
        }
    }

    pub async fn handle_new_video(
        &self,
        video_key: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // 1. Prepare temp file for analysis
        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path().to_path_buf();

        // 2. Download source video to temp
        self.storage.download(video_key, &temp_path).await?;

        // 3. Analyze video
        let video = AV::from_path(&temp_path)
            .await
            .map_err(|e| format!("Failed to analyze video: {:?}", e))?;

        let video_id = Uuid::new_v4().to_string();
        let file_stem = PathBuf::from(video_key)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        // HLS directory structure (logical path in storage)
        let hls_dir_key = PathBuf::from("hls").join(&file_stem);

        let segment_count = if video.segments.len() > 1 {
            video.segments.len() - 1
        } else {
            0
        };

        if segment_count == 0 {
            return Err("No segments found in video".into());
        }

        let mut segment_durations = Vec::with_capacity(segment_count);
        for i in 0..segment_count {
            let duration = video.segments[i + 1] - video.segments[i];
            segment_durations.push(duration);
        }

        let status = VideoStatus {
            id: video_id.clone(),
            source_path: PathBuf::from(video_key), // Key is the source
            hls_dir: hls_dir_key.clone(),
            total_segments: segment_count,
            segment_durations: segment_durations.clone(),
        };

        // 4. Save Status
        self.repo.save_video_status(&status).await?;

        // 5. Enqueue Segments
        for i in 0..segment_count {
            let job = SegmentJob {
                id: Uuid::new_v4().to_string(),
                video_id: video_id.clone(),
                segment_index: i,
                source_path: PathBuf::from(video_key), // Source is the key
                output_path: hls_dir_key.join(format!("segment_{}.mp4", i)), // Dest key
                start_time: video.segments[i],
                duration: segment_durations[i],
            };
            self.queue.enqueue_job(Job::Segment(job)).await?;
        }

        // 6. Enqueue Thumbnail Job
        let thumbnail_job = ThumbnailStripJob {
            id: Uuid::new_v4().to_string(),
            video_id: video_id.clone(),
            source_path: PathBuf::from(video_key),
            output_path: hls_dir_key.join("thumbnails.jpg"),
            interval_seconds: 5,
            width: 160,
        };
        self.queue
            .enqueue_job(Job::ThumbnailStrip(thumbnail_job))
            .await?;

        println!(
            "Enqueued {} segments + thumbnails for video {} ({})",
            segment_count, video_id, file_stem
        );

        Ok(video_id)
    }
}

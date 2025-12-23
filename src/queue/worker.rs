use crate::av::av::AV;
use crate::av::segments::transcode_at;
use crate::hls::MediaPlaylist;
use crate::queue::job::SegmentJob;
use crate::queue::redis_queue::RedisQueue;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// Number of concurrent worker tasks processing segment jobs
pub const WORKERS_COUNT: usize = 15;

/// Worker pool for processing segment transcoding jobs
pub struct WorkerPool {
    queue: Arc<RedisQueue>,
}

impl WorkerPool {
    /// Create a new worker pool
    pub fn new(queue: Arc<RedisQueue>) -> Self {
        Self { queue }
    }

    /// Start the worker pool - spawns WORKERS_COUNT tasks
    /// Returns handles to all worker tasks
    pub fn start(&self) -> Vec<JoinHandle<()>> {
        (0..WORKERS_COUNT)
            .map(|id| {
                let queue = self.queue.clone();
                tokio::spawn(async move {
                    worker_loop(id, queue).await;
                })
            })
            .collect()
    }
}

/// Main worker loop - blocks on BRPOP waiting for jobs
async fn worker_loop(worker_id: usize, queue: Arc<RedisQueue>) {
    println!("[Worker {}] Started", worker_id);

    loop {
        // BRPOP blocks here until a job is available (0 = infinite timeout)
        match queue.dequeue_segment(0.0).await {
            Ok(Some(job)) => {
                let priority = if job.segment_index < 2 {
                    "HIGH"
                } else {
                    "normal"
                };
                println!(
                    "[Worker {}] Processing segment {} ({} priority) for video {}",
                    worker_id, job.segment_index, priority, job.video_id
                );

                // Process the segment
                if let Err(e) = process_segment(&job).await {
                    eprintln!(
                        "[Worker {}] Failed to process segment {}: {:?}",
                        worker_id, job.segment_index, e
                    );
                    // TODO: Add retry logic or dead letter queue
                    continue;
                }

                // Check if all segments are complete
                if let Err(e) = check_video_completion(&queue, &job).await {
                    eprintln!(
                        "[Worker {}] Failed to check completion for video {}: {:?}",
                        worker_id, job.video_id, e
                    );
                }
            }
            Ok(None) => {
                // Timeout - continue waiting
                continue;
            }
            Err(e) => {
                eprintln!("[Worker {}] Error dequeuing job: {:?}", worker_id, e);
                // Brief pause before retrying on error
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

/// Process a single segment transcoding job
async fn process_segment(job: &SegmentJob) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create a minimal AV struct for transcoding
    // We need the path and segment times
    let av = AV::from_path(&job.source_path).await?;

    // Ensure output directory exists
    if let Some(parent) = job.output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Transcode the segment
    transcode_at(&av, job.segment_index, job.output_path.clone()).await;

    println!(
        "Segment {} transcoded to {:?}",
        job.segment_index, job.output_path
    );

    Ok(())
}

/// Check if all segments for a video are complete, and generate playlist if so
async fn check_video_completion(
    queue: &RedisQueue,
    job: &SegmentJob,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let completed = queue.mark_segment_complete(&job.video_id).await?;
    let total = queue.get_total_segments(&job.video_id).await?;

    println!("Video {} progress: {}/{}", job.video_id, completed, total);

    if completed == total {
        println!("Video {} complete! Generating playlist...", job.video_id);
        generate_playlist(queue, &job.video_id).await?;

        // Clean up Redis data
        queue.cleanup_video(&job.video_id).await?;
    }

    Ok(())
}

/// Generate the HLS playlist after all segments are complete
async fn generate_playlist(
    queue: &RedisQueue,
    video_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::av::segments::generate_init_segment;

    let status = queue
        .get_video_status(video_id)
        .await?
        .ok_or("Video status not found")?;

    let mut playlist = MediaPlaylist::new(0);
    playlist.playlist_type = Some("VOD".to_string());
    playlist.independent_segments = true;

    // Generate init segment from original source
    let init_path = status.hls_dir.join("init.mp4");

    if let Err(e) = generate_init_segment(&status.source_path, &init_path).await {
        eprintln!("Warning: Could not generate init segment: {:?}", e);
    } else {
        playlist.init_segment = Some("init.mp4".to_string());
    }

    let mut max_duration = 0.0;

    for (i, &duration) in status.segment_durations.iter().enumerate() {
        if duration > max_duration {
            max_duration = duration;
        }

        let segment_filename = format!("segment_{}.mp4", i);
        playlist.add_segment(duration, segment_filename);
    }

    playlist.target_duration = max_duration.ceil() as u64;

    let playlist_path = status.hls_dir.join("playlist.m3u8");
    playlist.write_to(&playlist_path).await?;

    println!("Playlist written to {:?}", playlist_path);

    Ok(())
}

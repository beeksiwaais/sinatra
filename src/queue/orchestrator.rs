use crate::av::av::AV;
use crate::queue::job::{SegmentJob, VideoStatus};
use crate::queue::redis_queue::{QueueError, RedisQueue};
use std::path::Path;
use uuid::Uuid;

/// Enqueue a video for processing - creates segment jobs for each segment
pub async fn enqueue_video(queue: &RedisQueue, path: &Path) -> Result<String, QueueError> {
    // Parse video to get segment information
    let path_buf = path.to_path_buf();
    let video = AV::from_path(&path_buf).await.map_err(|e| {
        QueueError::Serialization(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("{:?}", e),
        )))
    })?;

    let video_id = Uuid::new_v4().to_string();
    let file_stem = path.file_stem().unwrap().to_str().unwrap();
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| String::from("~/"));
    let hls_dir = std::path::PathBuf::from(upload_dir)
        .join("hls")
        .join(file_stem);

    let segment_count = if video.segments.len() > 1 {
        video.segments.len() - 1
    } else {
        0
    };

    if segment_count == 0 {
        return Err(QueueError::Serialization(serde_json::Error::io(
            std::io::Error::new(std::io::ErrorKind::Other, "No segments found in video"),
        )));
    }

    // Calculate durations for each segment
    let mut segment_durations = Vec::with_capacity(segment_count);
    for i in 0..segment_count {
        let duration = video.segments[i + 1] - video.segments[i];
        segment_durations.push(duration);
    }

    // Store video status in Redis
    let status = VideoStatus {
        id: video_id.clone(),
        source_path: path.to_path_buf(),
        hls_dir: hls_dir.clone(),
        total_segments: segment_count,
        segment_durations: segment_durations.clone(),
    };
    queue.set_video_status(&status).await?;

    // Enqueue each segment as individual job
    for i in 0..segment_count {
        let job = SegmentJob {
            id: Uuid::new_v4().to_string(),
            video_id: video_id.clone(),
            segment_index: i,
            source_path: path.to_path_buf(),
            output_path: hls_dir.join(format!("segment_{}.mp4", i)),
            start_time: video.segments[i],
            duration: segment_durations[i],
        };
        queue.enqueue_segment(&job).await?;
    }

    println!(
        "Enqueued {} segments for video {} ({})",
        segment_count, video_id, file_stem
    );

    Ok(video_id)
}

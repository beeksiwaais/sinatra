use crate::domain::av::av::AV;
use crate::domain::av::segments::{generate_init_segment, transcode_at};
use crate::domain::av::thumbnails::generate_strip;
use crate::domain::hls::MediaPlaylist;
use crate::domain::jobs::{Job, SegmentJob, ThumbnailStripJob};
use crate::ports::queue::JobQueuePort;
use crate::ports::repository::VideoStateRepository;
use crate::ports::storage::StoragePort;
use tempfile::NamedTempFile;

pub struct WorkerService<S, Q, R> {
    storage: S,
    queue: Q,
    repo: R,
}

impl<S, Q, R> WorkerService<S, Q, R>
where
    S: StoragePort + Clone + 'static,
    Q: JobQueuePort + Clone + 'static,
    R: VideoStateRepository + Clone + 'static,
{
    pub fn new(storage: S, queue: Q, repo: R) -> Self {
        Self {
            storage,
            queue,
            repo,
        }
    }

    pub async fn run_worker_loop(&self, worker_id: usize) {
        println!("[Worker {}] Started", worker_id);
        loop {
            match self.queue.dequeue_job(0.0).await {
                Ok(Some(job)) => {
                    if let Err(e) = self.process_job(&job, worker_id).await {
                        eprintln!("[Worker {}] Job failed: {:?}", worker_id, e);
                    }
                }
                Ok(None) => continue,
                Err(e) => {
                    eprintln!("[Worker {}] Queue error: {:?}", worker_id, e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn process_job(
        &self,
        job: &Job,
        worker_id: usize,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match job {
            Job::Segment(seg) => self.process_segment(seg, worker_id).await,
            Job::ThumbnailStrip(thumb) => self.process_thumbnail(thumb, worker_id).await,
        }
    }

    async fn process_segment(
        &self,
        job: &SegmentJob,
        worker_id: usize,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!(
            "[Worker {}] Processing segment {}",
            worker_id, job.segment_index
        );

        // 1. Prepare Paths
        let source_key = job.source_path.to_str().ok_or("Invalid source path")?;
        let dest_key = job.output_path.to_str().ok_or("Invalid output path")?;

        let temp_in = NamedTempFile::new()?;
        let _temp_out = NamedTempFile::new()?.into_temp_path(); // We need a path, but file might be recreated by ffmpeg?
                                                                // Actually ffmpeg creates the file. So we define a path in temp dir.
        let temp_out_path =
            std::env::temp_dir().join(format!("seg_{}_{}.mp4", job.video_id, job.segment_index));

        // 2. Download
        self.storage.download(source_key, temp_in.path()).await?;

        // 3. Transcode
        // We need AV from local file
        let av = AV::from_path(temp_in.path()).await?;
        transcode_at(&av, job.segment_index, temp_out_path.clone()).await;

        // 4. Upload
        if temp_out_path.exists() {
            self.storage.upload(&temp_out_path, dest_key).await?;
            tokio::fs::remove_file(&temp_out_path).await?;
        } else {
            return Err("Transcoding failed to produce output".into());
        }

        // 5. Update State
        self.check_video_completion(&job.video_id, source_key)
            .await?;

        Ok(())
    }

    async fn process_thumbnail(
        &self,
        job: &ThumbnailStripJob,
        worker_id: usize,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("[Worker {}] Processing thumbnails", worker_id);

        let source_key = job.source_path.to_str().ok_or("Invalid source path")?;
        let dest_key = job.output_path.to_str().ok_or("Invalid output path")?;

        let temp_in = NamedTempFile::new()?;
        let temp_out_path = std::env::temp_dir().join(format!("thumb_{}.jpg", job.video_id));

        self.storage.download(source_key, temp_in.path()).await?;

        generate_strip(
            temp_in.path(),
            &temp_out_path,
            job.interval_seconds,
            job.width,
        )
        .await?;

        if temp_out_path.exists() {
            self.storage.upload(&temp_out_path, dest_key).await?;
            tokio::fs::remove_file(&temp_out_path).await?;
        }

        Ok(())
    }

    async fn check_video_completion(
        &self,
        video_id: &str,
        source_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let completed = self.repo.mark_segment_complete(video_id).await?;
        let total = self.repo.get_total_segments(video_id).await?;

        println!("Video {} progress: {}/{}", video_id, completed, total);

        if completed == total {
            println!("Video {} complete! Generating playlist...", video_id);
            self.generate_playlist(video_id, source_key).await?;
            self.repo.cleanup_video(video_id).await?;
        }
        Ok(())
    }

    async fn generate_playlist(
        &self,
        video_id: &str,
        source_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let status = self
            .repo
            .get_video_status(video_id)
            .await?
            .ok_or("No status")?;

        let mut playlist = MediaPlaylist::new(0);
        playlist.playlist_type = Some("VOD".to_string());
        playlist.independent_segments = true;

        // Generate init segment
        // Need to download source again? Or cache? For simplicity, download again.
        let temp_in = NamedTempFile::new()?;
        self.storage.download(source_key, temp_in.path()).await?;

        let temp_init_path = std::env::temp_dir().join(format!("init_{}.mp4", video_id));
        if let Err(e) = generate_init_segment(temp_in.path(), &temp_init_path).await {
            eprintln!("Init segment gen failed: {:?}", e);
        } else {
            // Upload init.mp4
            let init_key = status.hls_dir.join("init.mp4");
            self.storage
                .upload(&temp_init_path, init_key.to_str().unwrap())
                .await?;
            playlist.init_segment = Some("init.mp4".to_string());
            let _ = tokio::fs::remove_file(&temp_init_path).await;
        }

        // Playlist construction
        let mut max_duration = 0.0;
        for (i, &duration) in status.segment_durations.iter().enumerate() {
            if duration > max_duration {
                max_duration = duration;
            }
            playlist.add_segment(duration, format!("segment_{}.mp4", i));
        }
        playlist.target_duration = max_duration.ceil() as u64;

        let temp_pl_path = std::env::temp_dir().join(format!("playlist_{}.m3u8", video_id));
        playlist.write_to(&temp_pl_path).await?;

        let pl_key = status.hls_dir.join("playlist.m3u8");
        self.storage
            .upload(&temp_pl_path, pl_key.to_str().unwrap())
            .await?;
        let _ = tokio::fs::remove_file(&temp_pl_path).await;

        Ok(())
    }
}

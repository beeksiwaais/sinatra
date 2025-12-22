use crate::av::av::AV;
use crate::av::segments::transcode_at;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub const MAX_CONCURRENT_VIDEOS: usize = 3;

pub async fn add_to_queue(semaphore: Arc<Semaphore>, path: &PathBuf) {
    tokio::spawn(process_video(
        semaphore,
        path.clone(), // bad
    ));
}

pub async fn process_video(semaphore: Arc<Semaphore>, path: PathBuf) {
    let permit = semaphore.clone().acquire_owned().await.unwrap();

    match AV::from_path(&path).await {
        Ok(video) => {
            let file_stem = path.file_stem().unwrap().to_str().unwrap();
            let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| String::from("~/"));
            let hls_dir = PathBuf::from(upload_dir).join("hls").join(file_stem);

            if let Err(e) = tokio::fs::create_dir_all(&hls_dir).await {
                eprintln!("Failed to create HLS directory: {:?}", e);
                return;
            }

            let mut segments = Vec::new();
            let mut n: usize = 1;
            while n < video.segments.len() - 1 {
                let segment_filename = format!("segment_{}.mp4", n);
                let segment_path = hls_dir.join(&segment_filename);

                transcode_at(&video, n, segment_path.clone()).await;
                segments.push(segment_filename);
                n += 1;
            }

            use crate::hls::MediaPlaylist;
            let mut playlist = MediaPlaylist::new(10);
            for segment in segments {
                playlist.add_segment(10.0, segment);
            }
            let playlist_path = hls_dir.join("playlist.m3u8");
            let _ = playlist.write_to(&playlist_path).await;
        }
        Err(e) => eprintln!("Error processing video: {:?}", e),
    }

    drop(permit);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::{mock, predicate::*};
    use tokio::test;
}

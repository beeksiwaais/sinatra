use std::path::{Path, PathBuf};
use crate::av::av::AV;
use crate::av::segments::transcode_at;
use tokio::sync::Semaphore;
use std::sync::Arc;

const MAX_CONCURRENT_VIDEOS: usize = 3;


pub async fn add_to_queue(path: &PathBuf) {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_VIDEOS));

    tokio::spawn(process_video(
        semaphore,
        path.clone() // bad
    ));
}

pub async fn process_video(semaphore: Arc<Semaphore>, path: PathBuf) {
    let permit = semaphore.clone().acquire_owned().await.unwrap();

    match AV::from_path(&path).await {
        Ok(video) => {
            if video.segments.is_empty() {
                eprintln!("Error: No segments found for video {:?}", path);
            } else {
                let mut segments_data: Vec<(PathBuf, f64)> = Vec::new();
                for i in 0..video.segments.len() {
                    let name = format!("_{}.mp4", i);
                    let segment_path = rename(&path, name);
                    
                    match transcode_at(&video, i, segment_path.clone()).await {
                        Ok(duration) => {
                            segments_data.push((segment_path, duration));
                        }
                        Err(e) => {
                            eprintln!("Error transcoding segment {} for video {:?}: {}", i, path, e);
                            // Decide if one failed segment should stop the whole video processing.
                            // For now, we'll skip this segment and try to make a playlist with the rest.
                        }
                    }
                }
                
                if !segments_data.is_empty() {
                    if let Err(e) = create_hls_playlist(&segments_data).await {
                        eprintln!("Error creating HLS playlist for video {:?}: {}", path, e);
                    }
                } else {
                    eprintln!("No segments were successfully transcoded for video {:?}", path);
                }
            }
        },
        Err(e) => eprintln!("Error processing video {:?}: {}", path, e),
    }

    drop(permit);
}

async fn create_hls_playlist(segments_with_durations: &[(PathBuf, f64)]) -> Result<(), std::io::Error> {
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    if segments_with_durations.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Segment list is empty"));
    }

    let first_segment_path = &segments_with_durations[0].0;
    let parent_dir = first_segment_path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Parent directory not found for segment")
    })?;

    let playlist_path = parent_dir.join("playlist.m3u8");
    let mut file = File::create(&playlist_path).await?;

    file.write_all(b"#EXTM3U\n").await?;
    file.write_all(b"#EXT-X-VERSION:3\n").await?;

    let max_duration = segments_with_durations
        .iter()
        .map(|(_, dur)| *dur)
        .fold(0.0f64, |acc, item| acc.max(item));
    
    let target_duration = if max_duration > 0.0 { max_duration.ceil() as u32 } else { 10 }; // Default to 10 if no segments or all have 0 duration

    file.write_all(format!("#EXT-X-TARGETDURATION:{}\n", target_duration).as_bytes()).await?;
    file.write_all(b"#EXT-X-MEDIA-SEQUENCE:0\n").await?; // Assuming sequence always starts at 0 for simplicity

    for (segment_path, duration) in segments_with_durations {
        file.write_all(format!("#EXTINF:{:.3},\n", duration).as_bytes()).await?;
        
        let filename_os_str = segment_path.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Segment path does not have a filename")
        })?;
        let filename_str = filename_os_str.to_str().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Segment filename is not valid UTF-8")
        })?;
        file.write_all(filename_str.as_bytes()).await?;
        file.write_all(b"\n").await?;
    }

    file.write_all(b"#EXT-X-ENDLIST\n").await?;

    Ok(())
}

fn rename(path: impl AsRef<Path>, name: String) -> PathBuf {
    let path = path.as_ref();
    let mut result = path.to_owned();
    let newfilename = path.file_name().unwrap().to_str().unwrap().to_owned() + &name.to_owned();
    result.set_file_name(newfilename);
    if let Some(ext) = path.extension() {
        result.set_extension(ext);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;
    use mockall::{predicate::*, mock};
    use tempfile::tempdir; // For temporary directory
    use tokio::fs; // For async file operations

    #[tokio::test]
    async fn test_create_hls_playlist_success() {
        // 1. Setup: Create a temporary directory
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let temp_dir_path = temp_dir.path();

        // 2. Setup: Prepare sample segments_with_durations
        let segments_data = vec![
            (temp_dir_path.join("segment_0.ts"), 9.500),
            (temp_dir_path.join("segment_1.ts"), 10.000),
            (temp_dir_path.join("segment_2.ts"), 8.000),
        ];

        // 3. Execution: Call create_hls_playlist
        let result = create_hls_playlist(&segments_data).await;

        // 4.a. Assertions: Check if create_hls_playlist returned Ok(())
        assert!(result.is_ok(), "create_hls_playlist failed: {:?}", result.err());

        // 4.b. Construct the expected path to playlist.m3u8
        let playlist_path = temp_dir_path.join("playlist.m3u8");
        assert!(playlist_path.exists(), "Playlist file was not created at {:?}", playlist_path);

        // 4.c. Read the content of the generated playlist.m3u8 file
        let playlist_content = fs::read_to_string(&playlist_path)
            .await
            .expect("Failed to read playlist file");

        // 4.d. Assert that the content matches the expected M3U8 string
        let expected_content = concat!(
            "#EXTM3U\n",
            "#EXT-X-VERSION:3\n",
            "#EXT-X-TARGETDURATION:10\n", // Max duration is 10.0, ceil is 10
            "#EXT-X-MEDIA-SEQUENCE:0\n",
            "#EXTINF:9.500,\n",
            "segment_0.ts\n",
            "#EXTINF:10.000,\n",
            "segment_1.ts\n",
            "#EXTINF:8.000,\n",
            "segment_2.ts\n",
            "#EXT-X-ENDLIST\n"
        );

        assert_eq!(playlist_content, expected_content, "Playlist content does not match expected");

        // 5. Teardown: temp_dir will be automatically cleaned up when it goes out of scope.
    }

    #[tokio::test]
    async fn test_create_hls_playlist_empty_segments() {
        let segments_data: Vec<(PathBuf, f64)> = vec![];
        let result = create_hls_playlist(&segments_data).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
        }
    }
}
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
            let mut segments = Vec::new();
            let mut n: usize = 1;
            while n < video.segments.len() {
                let name = format!("_{:?}.mp4", n);
                let segment_name = rename(&path, name);
                
                transcode_at(&video, n, segment_name.clone()).await;
                segments.push(segment_name);
                n += 1;
            }
            
            create_hls_playlist(&segments).await;
        },
        Err(e) => eprintln!("Error processing video: {:?}", e),
    }

    drop(permit);
}

async fn create_hls_playlist(segments: &[PathBuf]) -> Result<(), std::io::Error> {
    use std::env;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| String::from("~/"));
    let playlist_path = PathBuf::from(upload_dir).join("playlist.m3u8");

    let mut file = File::create(&playlist_path).await?;

    file.write_all(b"#EXTM3U\n").await?;
    file.write_all(b"#EXT-X-VERSION:3\n").await?;
    file.write_all(b"#EXT-X-TARGETDURATION:10\n").await?;
    file.write_all(b"#EXT-X-MEDIA-SEQUENCE:0\n").await?;

    for segment in segments {
        file.write_all(b"#EXTINF:10.0,\n").await?;
        file.write_all(segment.file_name().unwrap().to_str().unwrap().as_bytes()).await?;
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
}
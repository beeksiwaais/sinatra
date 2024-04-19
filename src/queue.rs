use std::path::{Path, PathBuf};
use tracing_subscriber::fmt::format;
use crate::video::av::AV;
use crate::video::segments::transcode_at;

pub async fn process_video(path: &PathBuf) {
    match AV::from_path(&path).await {
        Ok(video) => {
            let mut n: usize = 1;
            while n < video.segments.len() {
                let name = format!("_{:?}.mp4", n);
                let segment_name = rename(path, name);
                
                transcode_at(&video, n, segment_name).await;
                n = n+1;
            }
        },
        Err(e) => eprintln!("Erreur lors du traitement de la vidéo : {:?}", e),
    }
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
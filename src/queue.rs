use std::path::{Path, PathBuf};
use tracing_subscriber::fmt::format;
use crate::video::video::Video;

pub async fn process_video(path: &PathBuf) {
    let video = Video::from_path(&path).await.expect("TODO: panic message");
    println!("{:?}", video);

    //let uploads_dir = std::path::PathBuf(UPLOADS_DIR)
    let mut n: usize = 1;
    while n < video.segments.len() {
        println!("Requesting transcode for {:?}", n);
        let name = format!("_{:?}.mp4", n);
        let segment_name = rename(path, name);
        video.transcode_at(n, segment_name).await;
        n = n+1;
    }
    
    // call the queue to handle segmenting 
    
    // we generate the HLS m3u8 here and store it 
    // the we append the id to the b-tree to indicate that is not available yet
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
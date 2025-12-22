use crate::av::av::AV;
use ffmpeg_next as ffmpeg;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::task;

pub async fn get_segments(path: &PathBuf) -> Vec<f64> {
    let path_clone = path.clone();

    task::spawn_blocking(move || {
        ffmpeg::init().unwrap();
        match ffmpeg::format::input(&path_clone) {
            Ok(mut context) => {
                let stream_index = context
                    .streams()
                    .best(ffmpeg::media::Type::Video)
                    .map(|stream| stream.index());

                if let Some(stream_index) = stream_index {
                    let time_base = context.stream(stream_index).unwrap().time_base();
                    let time_base_f64 =
                        time_base.numerator() as f64 / time_base.denominator() as f64;

                    let mut segments = Vec::new();

                    for (stream, packet) in context.packets() {
                        if stream.index() == stream_index && packet.is_key() {
                            if let Some(pts) = packet.pts() {
                                let time = pts as f64 * time_base_f64;
                                segments.push(time);
                            }
                        }
                    }
                    segments
                } else {
                    eprintln!("No video stream found");
                    Vec::new()
                }
            }
            Err(e) => {
                eprintln!("Error opening input: {}", e);
                Vec::new()
            }
        }
    })
    .await
    .unwrap()
}

pub async fn transcode_at(av: &AV<'_>, segment: usize, at_path: PathBuf) {
    if segment >= av.segments.len() {
        println!(
            "Segment {:?} was not transcoded because it do not match known segments in av",
            segment
        );
    }

    let start_at = av.segments.get(segment).unwrap().to_string();
    let duration: f64 = av.segments.get(segment + 1).unwrap() - av.segments.get(segment).unwrap();
    let duration_as_str: String = duration.to_string();

    let transcode = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(&start_at)
        //.arg("-itsoffset")
        //.arg(&start_at)
        .arg("-i")
        .arg(av.path)
        .arg("-t")
        .arg(&duration_as_str)
        //.arg("-codec")
        //.arg("copy")
        .arg(at_path.clone())
        .output()
        .await;

    println!("{:?}", transcode);
}

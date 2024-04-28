use std::path::PathBuf;
use std::process::Command;
use regex::Regex;
use crate::av::av::AV;

pub fn get_segments(path: &PathBuf) -> Vec<f64> {
    let output = Command::new("ffprobe")
        .arg("-select_streams")
        .arg("v")
        .arg("-skip_frame")
        .arg("nokey")
        .arg("-show_frames")
        .arg("-v")
        .arg("quiet")
        .arg(path)
        .output();

    let segment = output.unwrap().stdout;
    let re = Regex::new(r"pts_time=(\d+\.\d+)").unwrap();
    let stdout_str = &*String::from_utf8_lossy(&segment);

    return stdout_str.lines()
        .filter_map(|line| {
            if re.is_match(line) {
                let caps = re.captures(line).unwrap();
                println!("{:?}", caps);
                Some(caps.get(1).unwrap().as_str())
            } else {
                None
            }
        })
        // Print the processed lines
        .map(|processed_line| {
            println!("{}", processed_line);
            processed_line.parse::<f64>().unwrap()
        })
        .collect();
}

pub async fn transcode_at(av: &AV<'_>, segment: usize, at_path: PathBuf) {
    if segment >= av.segments.len() {
        println!("Segment {:?} was not transcoded because it do not match known segments in av", segment);
    }

    let start_at = av.segments.get(segment).unwrap().to_string();
    let duration: f64 = av.segments.get(segment +1).unwrap() - av.segments.get(segment).unwrap();
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
        .output();

    println!("{:?}", transcode);
}
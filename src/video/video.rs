use std::any::Any;
use serde_json::{Result, to_string, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use futures::TryFutureExt;
use crate::video::audio_stream::AudioStream;
use crate::video::video_stream::VideoStream;
use regex::Regex;
use std::ffi::OsStr;


#[derive(Debug)]
pub(crate) struct Video<'a> {
    pub path: &'a Path,
    pub video_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub segments: Vec<f64>,
}

impl Video<'_> {
    pub async fn from_path(path: &PathBuf) -> Result<Video> {
        let streams = Self::get_streams(path);
        let segments = Self::get_segments(&path);

        Ok(Video {
            path,
            video_streams: streams.iter()
                .map(|stream| VideoStream::parse_stream(&stream))
                .flatten()
                .map(|stream| *stream)
                .collect(),
            audio_streams: streams.iter()
                .map(|stream| AudioStream::parse_stream(&stream))
                .flatten()
                .map(|stream| *stream)
                .collect(),
            segments,
        })
    }

    pub async fn transcode_at(self: &Self, segment: usize, at_path: PathBuf) {
        if segment >= self.segments.len() {
            println!("Segment {:?} was not transcoded because it do not match known segments in video", segment);
        }

        let start_at = self.segments.get(segment).unwrap().to_string();
        let duration: f64 = self.segments.get(segment +1).unwrap() - self.segments.get(segment).unwrap();
        let duration_as_str: String = duration.to_string();

        let transcode = Command::new("ffmpeg")
            .arg("-y")
            .arg("-ss")
            .arg(&start_at)
            //.arg("-itsoffset")
            //.arg(&start_at)
            .arg("-i")
            .arg(self.path)
            .arg("-t")
            .arg(&duration_as_str)
            //.arg("-codec")
            //.arg("copy")
            .arg(at_path.clone())
            .output();

        println!("{:?}", transcode);
    }

    pub async fn generate_m3u8 (self: &Self) {
        // tpdp
    }

    fn get_streams(path: &PathBuf) -> Vec<Value> {
        let probe = Command::new("ffprobe")
                .arg("-v")
                .arg("error")
                .arg("-show_format")
                .arg("-show_streams")
                .arg("-print_format")
                .arg("json")
                .arg(path)
                .output();

        let probe = probe.unwrap().stdout;
        let v: Value = serde_json::from_str(&*String::from_utf8_lossy(&probe)).unwrap();
        let duration = v.get("format").and_then(|format| format.get("duration")).and_then(|duration| duration.as_f64());

        if let Some(duration) = duration {
            if duration > 60.0 {
                panic!("Video too long")
            }
        }

        // todo check aspect_ratio

        let streams = v.get("streams").expect("Couldn't get streams from ffprobe");
        return streams.as_array().unwrap().clone();
    }

    fn get_segments(path: &PathBuf) -> Vec<f64> {
        print!("Hello from segment");
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
}
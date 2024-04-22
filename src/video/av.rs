use serde_json::Result;
use std::path::{Path, PathBuf};
use crate::video::stream::FromStream;
use crate::video::audio_stream::AudioStream;
use crate::video::video_stream::VideoStream;
use crate::video::stream::get_streams;
use crate::video::segments::get_segments;


#[derive(Debug)]
pub(crate) struct AV<'a> {
    pub path: &'a Path,
    pub video_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub segments: Vec<f64>,
}

impl AV<'_> {
    pub async fn from_path(path: &PathBuf) -> Result<AV> {
        let streams = get_streams(&path);

        Ok(AV {
            path,
            video_streams: streams.iter()
                .map(|stream| VideoStream::from_stream(&stream))
                .flatten()
                .map(|stream| *stream)
                .collect(),
            audio_streams: streams.iter()
                .map(|stream| AudioStream::from_stream(&stream))
                .flatten()
                .map(|stream| *stream)
                .collect(),
            segments: get_segments(&path),
        })
    }
}
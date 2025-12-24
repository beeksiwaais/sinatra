use super::audio_stream::AudioStream;
use super::segments::get_segments;
use super::stream::get_streams;
use super::stream::FromStream;
use super::video_stream::VideoStream;
use serde_json::Result;
use std::path::Path;

#[derive(Debug)]
pub(crate) struct AV<'a> {
    pub path: &'a Path,
    pub video_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub segments: Vec<f64>,
}

impl<'a> AV<'a> {
    pub async fn from_path(path: &'a Path) -> Result<AV<'a>> {
        let (streams, duration) = get_streams(path).await;

        let mut segments = get_segments(path).await;
        if let Some(&last) = segments.last() {
            if duration - last > 0.1 {
                segments.push(duration);
            }
        } else if duration > 0.0 {
            segments.push(0.0);
            segments.push(duration);
        }

        Ok(AV {
            path,
            video_streams: streams
                .iter()
                .map(|stream| VideoStream::from_stream(&stream))
                .flatten()
                .map(|stream| *stream)
                .collect(),
            audio_streams: streams
                .iter()
                .map(|stream| AudioStream::from_stream(&stream))
                .flatten()
                .map(|stream| *stream)
                .collect(),
            segments,
        })
    }
}

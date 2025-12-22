use crate::av::audio_stream::AudioStream;
use crate::av::segments::get_segments;
use crate::av::stream::get_streams;
use crate::av::stream::FromStream;
use crate::av::video_stream::VideoStream;
use serde_json::Result;
use std::path::{Path, PathBuf};

#[cfg(test)]
use mockall::{automock, predicate::*};

#[derive(Debug)]
pub(crate) struct AV<'a> {
    pub path: &'a Path,
    pub video_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub segments: Vec<f64>,
}

impl AV<'_> {
    pub async fn from_path(path: &PathBuf) -> Result<AV> {
        let (streams, duration) = get_streams(&path).await;

        let mut segments = get_segments(&path).await;
        // Only append duration if it's significantly greater than the last segment
        if let Some(&last) = segments.last() {
            if duration - last > 0.1 {
                segments.push(duration);
            }
        } else if duration > 0.0 {
            // If no keyframes found (weird), but duration exists, add 0.0 and duration?
            // Assuming at least 0.0 is returned by get_segments if there's video
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

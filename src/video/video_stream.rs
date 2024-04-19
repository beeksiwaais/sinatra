use serde_json::{Value};
use crate::video::stream::FromStream;
use std::option::Option;

#[derive(Debug)]
pub(crate) struct VideoStream {
    codec: String,
    profile: String,
    pix_fmt: String,
    color_space: String,
    frame_rate: String,
    bit_rate: String,
    width: u16,
    height: u16,
    aspect_ratio: String,
    is_horizontal: bool,
}

impl FromStream for VideoStream {
    fn from_stream(stream_data: &Value) -> Option<Box<Self>> {
        if let Some(codec_type) = stream_data.get("codec_type").and_then(|v| v.as_str()) {
            match codec_type {
                "video" => {
                    let width = stream_data.get("width")?.as_u64().unwrap_or(0) as u16;
                    let height = stream_data.get("height")?.as_u64().unwrap_or(0) as u16;
                    let is_horizontal = width > height;

                    Some(Box::new(VideoStream {
                        codec: stream_data.get("codec_name")?.to_string(),
                        profile: stream_data.get("profile")?.to_string(),
                        pix_fmt: stream_data.get("pix_fmt")?.to_string(),
                        color_space: stream_data.get("color_space")?.to_string(),
                        frame_rate: stream_data.get("frame_rate")?.to_string(),
                        bit_rate: stream_data.get("bit_rate")?.to_string(),
                        width,
                        height,
                        aspect_ratio: stream_data.get("aspect_ratio")?.to_string(),
                        is_horizontal,
                    }))
                }
                _ => None,
            }
        } else {
            None
        }
    }
}
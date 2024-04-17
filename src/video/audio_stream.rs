use serde_json::{Value};

#[derive(Debug)]
pub(crate) struct AudioStream {
    codec: String,
    profile: String,

    bit_rate: String,
}

impl AudioStream {
    pub fn parse_stream(stream_data: &Value) -> Option<Box<AudioStream>> {
        if let Some(codec_type) = stream_data.get("codec_type").and_then(|v| v.as_str()) {
            match codec_type {
                "audio" => {
                    Some(Box::new(AudioStream {
                        codec: stream_data.get("codec_name")?.to_string(),
                        profile: stream_data.get("profile")?.to_string(),
                        bit_rate: "".to_string(),
                    }))
                }
                _ => {
                    None
                }
            }
        } else {
            None
        }
    }
}
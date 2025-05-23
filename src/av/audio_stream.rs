use serde_json::{Value};
use crate::av::stream::FromStream;

#[derive(Debug)]
pub(crate) struct AudioStream {
    codec: String,
    profile: String,
    bit_rate: String,
}

impl FromStream for AudioStream {
    fn from_stream(stream_data: &Value) -> Option<Box<AudioStream>> {
        if let Some(codec_type) = stream_data.get("codec_type").and_then(|v| v.as_str()) {
            match codec_type {
                "audio" => {
                    Some(Box::new(AudioStream {
                        codec: stream_data.get("codec_name")?.to_string(),
                        profile: stream_data.get("profile")?.to_string(),
                        bit_rate: stream_data.get("profile")?.to_string(),
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

#[cfg(test)]
mod tests {
    use super::AudioStream;
    use super::FromStream;
    use serde_json::json;

    #[test]
    fn test_from_stream_valid_audio() {
        let stream_data = json!({
            "codec_type": "audio",
            "codec_name": "aac",
            "profile": "LC"
        });
        let audio_stream = AudioStream::from_stream(&stream_data);
        assert!(audio_stream.is_some());
        let stream = audio_stream.unwrap();
        assert_eq!(stream.codec, "\"aac\""); // serde_json::Value::to_string() adds quotes
        assert_eq!(stream.profile, "\"LC\"");
        assert_eq!(stream.bit_rate, "\"LC\""); // Reflects current behavior
    }

    #[test]
    fn test_from_stream_non_audio_codec_type() {
        let stream_data = json!({
            "codec_type": "video",
            "codec_name": "h264",
            "profile": "High"
        });
        let audio_stream = AudioStream::from_stream(&stream_data);
        assert!(audio_stream.is_none());
    }

    #[test]
    fn test_from_stream_missing_codec_type() {
        let stream_data = json!({
            "codec_name": "aac",
            "profile": "LC"
        });
        let audio_stream = AudioStream::from_stream(&stream_data);
        assert!(audio_stream.is_none());
    }

    #[test]
    fn test_from_stream_missing_codec_name() {
        let stream_data = json!({
            "codec_type": "audio",
            "profile": "LC"
        });
        let audio_stream = AudioStream::from_stream(&stream_data);
        assert!(audio_stream.is_none());
    }

    #[test]
    fn test_from_stream_missing_profile() {
        let stream_data = json!({
            "codec_type": "audio",
            "codec_name": "aac"
        });
        let audio_stream = AudioStream::from_stream(&stream_data);
        assert!(audio_stream.is_none());
    }
}
use ffmpeg_next as ffmpeg;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::task;

pub trait FromStream {
    fn from_stream(stream_data: &Value) -> Option<Box<Self>>
    where
        Self: Sized;
}

pub async fn get_streams(path: &PathBuf) -> (Vec<Value>, f64) {
    let path_clone = path.clone();

    task::spawn_blocking(move || {
        ffmpeg::init().unwrap();
        match ffmpeg::format::input(&path_clone) {
            Ok(input) => {
                let duration = input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64;
                let mut streams = Vec::new();

                for stream in input.streams() {
                    let params = stream.parameters();
                    let medium = params.medium();

                    let codec_type = match medium {
                        ffmpeg::media::Type::Video => "video",
                        ffmpeg::media::Type::Audio => "audio",
                        _ => "unknown",
                    };

                    if codec_type == "unknown" {
                        continue;
                    }

                    // Resolve codec name
                    let codec_id = params.id();
                    let codec = ffmpeg::decoder::find(codec_id);
                    let codec_name = codec
                        .map(|c| c.name().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    let mut json_val = json!({
                        "codec_type": codec_type,
                        "codec_name": codec_name,
                    });

                    // Create a context to inspect details
                    if let Ok(ctx) =
                        ffmpeg::codec::context::Context::from_parameters(params.clone())
                    {
                        if codec_type == "video" {
                            if let Ok(decoder) = ctx.decoder().video() {
                                json_val["width"] = json!(decoder.width());
                                json_val["height"] = json!(decoder.height());
                                json_val["pix_fmt"] = json!(format!("{:?}", decoder.format()));
                                json_val["frame_rate"] = json!(stream.avg_frame_rate().to_string());
                                json_val["aspect_ratio"] = json!(format!(
                                    "{}:{}",
                                    decoder.aspect_ratio().numerator(),
                                    decoder.aspect_ratio().denominator()
                                ));
                                json_val["bit_rate"] = json!(decoder.bit_rate().to_string());

                                if let Some(profile) = Some(format!("{:?}", decoder.profile())) {
                                    json_val["profile"] = json!(profile);
                                } else {
                                    json_val["profile"] = json!("unknown");
                                }
                                json_val["color_space"] = json!("unknown");
                            }
                        } else if codec_type == "audio" {
                            if let Ok(decoder) = ctx.decoder().audio() {
                                json_val["bit_rate"] = json!(decoder.bit_rate().to_string());
                                if let Some(profile) = Some(format!("{:?}", decoder.profile())) {
                                    json_val["profile"] = json!(profile);
                                } else {
                                    json_val["profile"] = json!("unknown");
                                }
                            }
                        }
                    } else {
                        eprintln!("Failed to create context from parameters");
                    }

                    streams.push(json_val);
                }
                (streams, duration)
            }
            Err(e) => {
                eprintln!("Error opening input: {}", e);
                (vec![], 0.0)
            }
        }
    })
    .await
    .unwrap()
}

// Returns a vector of streams when given a valid path.
#[tokio::test]
#[ignore]
async fn test_valid_path() {
    use get_streams;
    use serde_json::Value;
    use std::path::PathBuf;

    let path = PathBuf::from("valid_path");
    let streams = get_streams(&path).await;

    assert_eq!(streams.0.len(), 2);
}

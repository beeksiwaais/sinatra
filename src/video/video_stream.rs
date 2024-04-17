use serde_json::{Value};


#[derive(Debug)]
pub(crate) struct VideoStream {
    codec: String,
    // ex: h264
    profile: String,
    // ex: High
    pix_fmt: String,
    // ex: yuv420p
    color_space: String,
    frame_rate: String,
    bit_rate: String,

    // Video
    width: u8,
    height: u8,
    aspect_ratio: String,
    //ex: "9:16"
    is_horizontal: bool,
}

impl VideoStream {
    fn is_hdr(&self) -> bool {
        // HDR when color space = bt2020nc & color transfert & smpte2084 & color primary bt2020 ??
        return false;
    }

    fn is_vertical(&self) -> bool {
        if self.height > self.width {
            return true;
        }
        return false;
    }

    pub fn parse_stream(stream_data: &Value) -> Option<Box<VideoStream>> {
        if let Some(codec_type) = stream_data.get("codec_type").and_then(|v| v.as_str()) {
            match codec_type {
                "video" => {
                    Some(Box::new(VideoStream {
                        codec: stream_data.get("codec_name")?.to_string(),
                        profile: stream_data.get("profile")?.to_string(),
                        pix_fmt: stream_data.get("pix_fmt")?.to_string(),
                        color_space: stream_data.get("color_space")?.to_string(),
                        frame_rate: "".to_string(),
                        bit_rate: "".to_string(),
                        width: 0,
                        height: 0,
                        aspect_ratio: "".to_string(),
                        is_horizontal: false,
                    }))
                }
                _ => {
                    None
                }
            }
        } else {
            println!("Champ 'codec_type' manquant ou invalide dans les données du flux.");
            None
        }
    }
}
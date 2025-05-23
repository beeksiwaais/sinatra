use serde_json::{Value};
use crate::av::stream::FromStream;
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
                "av" => {
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

#[cfg(test)]
mod tests {
    use super::VideoStream;
    use crate::av::stream::FromStream;
    use serde_json::json;

    #[test]
    fn test_from_stream_valid_video() {
        let stream_data = json!({
            "codec_type": "av",
            "codec_name": "h264",
            "profile": "High",
            "pix_fmt": "yuv420p",
            "color_space": "bt709",
            "frame_rate": "30000/1001",
            "bit_rate": "5000000",
            "width": 1920,
            "height": 1080,
            "aspect_ratio": "16:9"
        });
        let video_stream = VideoStream::from_stream(&stream_data);
        assert!(video_stream.is_some());
        let stream = video_stream.unwrap();
        assert_eq!(stream.codec, "\"h264\"");
        assert_eq!(stream.profile, "\"High\"");
        assert_eq!(stream.pix_fmt, "\"yuv420p\"");
        assert_eq!(stream.color_space, "\"bt709\"");
        assert_eq!(stream.frame_rate, "\"30000/1001\"");
        assert_eq!(stream.bit_rate, "\"5000000\"");
        assert_eq!(stream.width, 1920);
        assert_eq!(stream.height, 1080);
        assert_eq!(stream.aspect_ratio, "\"16:9\"");
        assert_eq!(stream.is_horizontal, true);
    }

    #[test]
    fn test_from_stream_vertical_video() {
        let stream_data = json!({
            "codec_type": "av",
            "codec_name": "h264",
            "profile": "High",
            "pix_fmt": "yuv420p",
            "color_space": "bt709",
            "frame_rate": "30000/1001",
            "bit_rate": "5000000",
            "width": 1080,
            "height": 1920,
            "aspect_ratio": "9:16"
        });
        let video_stream = VideoStream::from_stream(&stream_data);
        assert!(video_stream.is_some());
        let stream = video_stream.unwrap();
        assert_eq!(stream.width, 1080);
        assert_eq!(stream.height, 1920);
        assert_eq!(stream.is_horizontal, false);
    }

    #[test]
    fn test_from_stream_square_video() {
        let stream_data = json!({
            "codec_type": "av",
            "codec_name": "h264",
            "profile": "High",
            "pix_fmt": "yuv420p",
            "color_space": "bt709",
            "frame_rate": "30000/1001",
            "bit_rate": "5000000",
            "width": 1080,
            "height": 1080,
            "aspect_ratio": "1:1"
        });
        let video_stream = VideoStream::from_stream(&stream_data);
        assert!(video_stream.is_some());
        let stream = video_stream.unwrap();
        assert_eq!(stream.width, 1080);
        assert_eq!(stream.height, 1080);
        assert_eq!(stream.is_horizontal, false); // width > height is false
    }

    #[test]
    fn test_from_stream_non_video_codec_type() {
        let stream_data = json!({
            "codec_type": "audio",
            "codec_name": "aac",
            // ... other fields that don't matter for this test
        });
        let video_stream = VideoStream::from_stream(&stream_data);
        assert!(video_stream.is_none());
    }

    #[test]
    fn test_from_stream_missing_codec_type() {
        let stream_data = json!({
            "codec_name": "h264",
            // ... other fields
        });
        let video_stream = VideoStream::from_stream(&stream_data);
        assert!(video_stream.is_none());
    }

    #[test]
    fn test_from_stream_missing_critical_fields() {
        let base_json = || json!({
            "codec_type": "av",
            "profile": "High",
            "pix_fmt": "yuv420p",
            "color_space": "bt709",
            "frame_rate": "30000/1001",
            "bit_rate": "5000000",
            "width": 1920,
            "height": 1080,
            "aspect_ratio": "16:9"
        });

        let fields_to_test = [
            "codec_name", "profile", "pix_fmt", "color_space", 
            "frame_rate", "bit_rate", "width", "height", "aspect_ratio"
        ];

        for &field in fields_to_test.iter() {
            let mut data = base_json();
            data["codec_name"] = json!("h264"); // Ensure codec_name is present unless it's the one being removed
            if field == "width" || field == "height" { // width/height are handled by unwrap_or(0), won't be None
                 // these are tested in test_from_stream_width_height_not_numbers
                continue;
            }
            data.as_object_mut().unwrap().remove(field);
            
            // Special case for width/height as they are not Option<String> but u16 with unwrap_or(0)
            // Their absence (null) would be caught by Option `?` before `as_u64` call.
            // If the field is literally absent from JSON, `get()` returns None.
             if field != "width" && field != "height" { // only check None for fields that cause it
                let video_stream = VideoStream::from_stream(&data);
                assert!(video_stream.is_none(), "Expected None when '{}' is missing", field);
            }
        }
        
        // Test for width and height specifically if the get() itself returns None (field truly absent)
        let mut data_no_width = base_json();
        data_no_width["codec_name"] = json!("h264");
        data_no_width.as_object_mut().unwrap().remove("width");
        assert!(VideoStream::from_stream(&data_no_width).is_none(), "Expected None when 'width' is completely missing");

        let mut data_no_height = base_json();
        data_no_height["codec_name"] = json!("h264");
        data_no_height.as_object_mut().unwrap().remove("height");
        assert!(VideoStream::from_stream(&data_no_height).is_none(), "Expected None when 'height' is completely missing");
    }

    #[test]
    fn test_from_stream_width_height_not_numbers() {
        let mut stream_data_invalid_width = json!({
            "codec_type": "av",
            "codec_name": "h264",
            "profile": "High",
            "pix_fmt": "yuv420p",
            "color_space": "bt709",
            "frame_rate": "30000/1001",
            "bit_rate": "5000000",
            "width": "not_a_number", // Invalid width
            "height": 1080,
            "aspect_ratio": "16:9"
        });
        let video_stream_invalid_width = VideoStream::from_stream(&stream_data_invalid_width);
        assert!(video_stream_invalid_width.is_some());
        let stream_iw = video_stream_invalid_width.unwrap();
        assert_eq!(stream_iw.width, 0); // unwrap_or(0)
        assert_eq!(stream_iw.height, 1080);
        assert_eq!(stream_iw.is_horizontal, false); // 0 > 1080 is false

        let mut stream_data_null_height = json!({
            "codec_type": "av",
            "codec_name": "h264",
            "profile": "High",
            "pix_fmt": "yuv420p",
            "color_space": "bt709",
            "frame_rate": "30000/1001",
            "bit_rate": "5000000",
            "width": 1920,
            "height": null, // Null height
            "aspect_ratio": "16:9"
        });
        let video_stream_null_height = VideoStream::from_stream(&stream_data_null_height);
        assert!(video_stream_null_height.is_some());
        let stream_nh = video_stream_null_height.unwrap();
        assert_eq!(stream_nh.width, 1920);
        assert_eq!(stream_nh.height, 0); // unwrap_or(0)
        assert_eq!(stream_nh.is_horizontal, true); // 1920 > 0 is true
        
        let mut stream_data_both_zero = json!({
            "codec_type": "av",
            "codec_name": "h264",
            "profile": "High",
            "pix_fmt": "yuv420p",
            "color_space": "bt709",
            "frame_rate": "30000/1001",
            "bit_rate": "5000000",
            "width": null, 
            "height": "zero", 
            "aspect_ratio": "16:9"
        });
        let video_stream_both_zero = VideoStream::from_stream(&stream_data_both_zero);
        assert!(video_stream_both_zero.is_some());
        let stream_bz = video_stream_both_zero.unwrap();
        assert_eq!(stream_bz.width, 0);
        assert_eq!(stream_bz.height, 0);
        assert_eq!(stream_bz.is_horizontal, false); // 0 > 0 is false
    }
}
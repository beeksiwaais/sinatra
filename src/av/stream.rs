use std::path::PathBuf;
use std::process::Command;
use serde_json::Value;

pub trait FromStream {
    fn from_stream(stream_data: &Value) -> Option<Box<Self>>
        where
            Self: Sized;
}

pub fn get_streams(path: &PathBuf) -> Vec<Value> {
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
    let duration = v
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(|duration| duration.as_f64());

    if let Some(duration) = duration {
        if duration > 60.0 {
            panic!("Video too long")
        }
    }

    let streams = v
        .get("streams")
        .expect("Couldn't get streams from ffprobe");

    return streams.as_array().unwrap().clone();
}

    // Returns a vector of streams when given a valid media file.
    #[test]
    fn test_get_streams_with_valid_media_file() {
        use std::path::PathBuf;
        // use serde_json::Value; // Removed unused import from test function
    
        let path = PathBuf::from("tests/assets/empty.mp4");
        let streams = get_streams(&path);
    
        assert_eq!(streams.len(), 2);
    }
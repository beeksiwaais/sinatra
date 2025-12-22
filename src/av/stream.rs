use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

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

    let streams = v.get("streams").expect("Couldn't get streams from ffprobe");

    return streams.as_array().unwrap().clone();
}

// Returns a vector of streams when given a valid path.
#[test]
#[ignore]
fn test_valid_path() {
    use get_streams;
    use serde_json::Value;
    use std::path::PathBuf;

    let path = PathBuf::from("valid_path");
    let streams = get_streams(&path);

    assert_eq!(streams.len(), 2);
}

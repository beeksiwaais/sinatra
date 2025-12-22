use serde_json::Value;
use std::path::PathBuf;
use tokio::process::Command;

pub trait FromStream {
    fn from_stream(stream_data: &Value) -> Option<Box<Self>>
    where
        Self: Sized;
}

pub async fn get_streams(path: &PathBuf) -> (Vec<Value>, f64) {
    let probe = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_format")
        .arg("-show_streams")
        .arg("-print_format")
        .arg("json")
        .arg(path)
        .output()
        .await;

    let probe = probe.unwrap().stdout;
    let v: Value = serde_json::from_str(&*String::from_utf8_lossy(&probe)).unwrap();
    let duration = v
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(|duration| duration.as_str())
        .and_then(|d_str| d_str.parse::<f64>().ok())
        .unwrap_or(0.0);

    if duration > 60.0 {
        panic!("Video too long")
    }

    let streams = v.get("streams").expect("Couldn't get streams from ffprobe");

    (streams.as_array().unwrap().clone(), duration)
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

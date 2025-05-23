use std::path::PathBuf;
use std::process::Command;
use serde_json::Value;

pub trait FromStream {
    fn from_stream(stream_data: &Value) -> Option<Box<Self>>
        where
            Self: Sized;
}

#[cfg_attr(test, mockall::automock)]
pub trait StreamProvider {
    fn provide_streams(&self, path: &PathBuf) -> Vec<Value>;
}

pub struct RealStreamProvider;

impl StreamProvider for RealStreamProvider {
    fn provide_streams(&self, path: &PathBuf) -> Vec<Value> {
        let probe_output_result = std::process::Command::new("ffprobe") // Using std::process::Command
            .arg("-v").arg("error")
            .arg("-show_format").arg("-show_streams")
            .arg("-print_format").arg("json")
            .arg(path)
            .output();

        let probe = match probe_output_result {
            Ok(out) => {
                if !out.status.success() {
                    eprintln!("ffprobe (streams) failed with status: {}. Stderr: {}", out.status, String::from_utf8_lossy(&out.stderr));
                    return Vec::new();
                }
                out
            },
            Err(e) => {
                eprintln!("Failed to execute ffprobe for streams: {}", e);
                return Vec::new();
            }
        };

        let v: Value = match serde_json::from_slice(&probe.stdout) { // from_slice for Vec<u8>
            Ok(val) => val,
            Err(e) => {
                eprintln!("Failed to parse ffprobe JSON for streams: {}", e);
                return Vec::new();
            }
        };
        
        if let Some(format_val) = v.get("format") { // Renamed to format_val to avoid conflict
            if let Some(duration_str) = format_val.get("duration").and_then(|d| d.as_str()) {
                if let Ok(duration) = duration_str.parse::<f64>() {
                    if duration > 60.0 {
                        // Consider returning Result from provide_streams to propagate this
                        panic!("Video too long (from RealStreamProvider): duration {}s", duration); 
                    }
                }
            }
        }
        
        match v.get("streams") {
            Some(streams_val) => streams_val.as_array().cloned().unwrap_or_else(Vec::new),
            None => {
                 eprintln!("'streams' field not found in ffprobe output");
                 Vec::new()
            }
        }
    }
}

// The old `get_streams` function is removed.

    // Returns a vector of streams when given a valid media file.
    #[test]
    fn test_get_streams_with_valid_media_file() {
        use std::path::PathBuf;
        
        let provider = RealStreamProvider;
        let path = PathBuf::from("tests/assets/empty.mp4"); 
        let streams = provider.provide_streams(&path);
    
        assert_eq!(streams.len(), 2);
    }
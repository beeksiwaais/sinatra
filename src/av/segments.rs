use std::path::PathBuf;
use std::process::Command;
use regex::Regex;
use crate::av::av::AV;

pub fn get_segments(path: &PathBuf) -> Vec<f64> {
    let output = Command::new("ffprobe")
        .arg("-select_streams")
        .arg("v")
        .arg("-skip_frame")
        .arg("nokey")
        .arg("-show_frames")
        .arg("-v")
        .arg("quiet")
        .arg(path)
        .output();

    let segment = output.unwrap().stdout;
    let re = Regex::new(r"pts_time=(\d+\.\d+)").unwrap();
    let stdout_str = &*String::from_utf8_lossy(&segment);

    return stdout_str.lines()
        .filter_map(|line| {
            if re.is_match(line) {
                let caps = re.captures(line).unwrap();
                println!("{:?}", caps);
                Some(caps.get(1).unwrap().as_str())
            } else {
                None
            }
        })
        // Print the processed lines
        .map(|processed_line| {
            println!("{}", processed_line);
            processed_line.parse::<f64>().unwrap()
        })
        .collect();
use std::error::Error;

// Custom error type (optional, but good practice)
#[derive(Debug)]
struct TranscodeError(String);

impl std::fmt::Display for TranscodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for TranscodeError {}

pub async fn transcode_at(av: &AV<'_>, segment_index: usize, at_path: PathBuf) -> Result<f64, Box<dyn Error>> {
    if segment_index >= av.segments.len() {
        let err_msg = format!("Segment index {:?} is out of bounds (len: {}). Not transcoding.", segment_index, av.segments.len());
        eprintln!("{}", err_msg);
        return Err(Box::new(TranscodeError(err_msg)));
    }

    let start_at = av.segments.get(segment_index).unwrap().to_string();
    let mut calculated_duration: f64 = 0.0; // Will be overwritten or used for return

    let mut command = Command::new("ffmpeg");
    command.arg("-y").arg("-ss").arg(&start_at).arg("-i").arg(av.path);

    let is_last_segment = segment_index == av.segments.len() - 1;

    if !is_last_segment {
        let end_at = av.segments.get(segment_index + 1).unwrap();
        let duration_val = *end_at - av.segments.get(segment_index).unwrap();
        if duration_val <= 0.0 {
            let warn_msg = format!("Warning: Calculated duration for segment {} is not positive ({}). Check segment times. Omitting -t.", segment_index, duration_val);
            println!("{}", warn_msg);
            // If duration is not positive, behave like the last segment and let ffmpeg run to end / use ffprobe
        } else {
            calculated_duration = duration_val;
            command.arg("-t").arg(calculated_duration.to_string());
        }
    } else {
        println!("Transcoding last segment {} from {} to end (will determine duration via ffprobe).", segment_index, start_at);
    }

    command.arg(at_path.clone());
    
    let transcode_process = command.output()?; // Propagates IO errors for command execution

    if !transcode_process.status.success() {
        let err_msg = format!("Error transcoding segment {}: {}", segment_index, String::from_utf8_lossy(&transcode_process.stderr));
        eprintln!("{}", err_msg);
        return Err(Box::new(TranscodeError(err_msg)));
    }
    
    println!("Successfully transcoded segment {} to {:?}", segment_index, at_path);

    if is_last_segment || calculated_duration <= 0.0 { // also use ffprobe if calculated duration was bad
        let at_path_str = at_path.to_str().ok_or_else(|| Box::new(TranscodeError("Path is not valid UTF-8".to_string())))?;
        let ffprobe_output = Command::new("ffprobe")
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("default=noprint_wrappers=1:nokey=1")
            .arg(at_path_str)
            .output()?;

        if !ffprobe_output.status.success() {
            let err_msg = format!("ffprobe error for {}: {}", at_path_str, String::from_utf8_lossy(&ffprobe_output.stderr));
            eprintln!("{}", err_msg);
            return Err(Box::new(TranscodeError(err_msg)));
        }

        let duration_str = String::from_utf8(ffprobe_output.stdout)?.trim().to_string();
        let actual_duration = duration_str.parse::<f64>()?;
        println!("Last segment ({}) or segment with non-positive calculated duration: ffprobed duration: {}", segment_index, actual_duration);
        Ok(actual_duration)
    } else {
        Ok(calculated_duration)
    }
}
use std::path::PathBuf; // Path, Output, io, async_trait, TokioCommand removed from top
use regex::Regex;
use crate::av::av::AV;
// These imports are now primarily within cmd_executor or tests

// The cmd_executor module has been moved to src/av/cmd.rs

// get_segments uses FfprobeRunner (previously SyncCommandExecutor)
// Note: The `cmd_executor` part of the path will be updated in a later step
// to `crate::av::cmd`
pub fn get_segments(path: &PathBuf, runner: &impl crate::av::cmd::FfprobeRunner) -> Vec<f64> { // Updated path
    let output_result = runner.run_ffprobe_for_segments(path);

    let output = match output_result {
        Ok(out) => out,
        Err(e) => {
            eprintln!("Error running ffprobe for segments: {}", e);
            return Vec::new(); 
        }
    };

    if !output.status.success() {
        eprintln!("ffprobe for segments command failed with status: {}", output.status);
        eprintln!("ffprobe stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Vec::new();
    }

    let segment_stdout = output.stdout;
    let re = Regex::new(r"pts_time=(\d+\.\d+)").unwrap();
    let stdout_str = &*String::from_utf8_lossy(&segment_stdout);

    return stdout_str.lines()
        .filter_map(|line| {
            if re.is_match(line) {
                let caps = re.captures(line).unwrap();
                Some(caps.get(1).unwrap().as_str())
            } else {
                None
            }
        })
        .map(|processed_line| {
            processed_line.parse::<f64>().unwrap()
        })
        .collect();
} 

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

// Updated transcode_at to be async and use the new TranscodeExecutor
pub async fn transcode_at(av: &AV<'_>, segment_index: usize, at_path: PathBuf, runner: &impl crate::av::cmd::TranscodeExecutor) -> Result<f64, Box<dyn Error>> { // Updated path
    if segment_index >= av.segments.len() {
        let err_msg = format!("Segment index {:?} is out of bounds (len: {}). Not transcoding.", segment_index, av.segments.len());
        eprintln!("{}", err_msg);
        return Err(Box::new(TranscodeError(err_msg)));
    }

    let start_at = av.segments.get(segment_index).unwrap().to_string();
    let mut calculated_duration_opt: Option<String> = None;
    let mut actual_calculated_duration_f64: f64 = 0.0;

    let is_last_segment = segment_index == av.segments.len() - 1;

    if !is_last_segment {
        let end_at = av.segments.get(segment_index + 1).unwrap();
        let duration_val = *end_at - av.segments.get(segment_index).unwrap();
        if duration_val <= 0.0 {
            let warn_msg = format!("Warning: Calculated duration for segment {} is not positive ({}). Check segment times. Omitting -t.", segment_index, duration_val);
            println!("{}", warn_msg);
        } else {
            actual_calculated_duration_f64 = duration_val;
            calculated_duration_opt = Some(duration_val.to_string());
        }
    } else {
        println!("Transcoding last segment {} from {} to end (will determine duration via ffprobe).", segment_index, start_at);
    }
    
    let transcode_process = runner.run_ffmpeg_transcode(
        av.path, 
        &start_at, 
        calculated_duration_opt, 
        &at_path
    ).await.map_err(|e| Box::new(TranscodeError(format!("ffmpeg command execution failed for segment {}: {}", segment_index, e))))?;

    if !transcode_process.status.success() {
        let err_msg = format!("Error transcoding segment {}: {}", segment_index, String::from_utf8_lossy(&transcode_process.stderr));
        eprintln!("{}", err_msg);
        return Err(Box::new(TranscodeError(err_msg)));
    }
    
    println!("Successfully transcoded segment {} to {:?}", segment_index, at_path);

    if is_last_segment || actual_calculated_duration_f64 <= 0.0 {
        let ffprobe_output = runner.run_ffprobe_for_duration(&at_path)
            .await
            .map_err(|e| Box::new(TranscodeError(format!("ffprobe duration command execution failed for {}: {}", at_path.display(), e))))?;

        if !ffprobe_output.status.success() {
            let err_msg = format!("ffprobe error for {}: {}", at_path.display(), String::from_utf8_lossy(&ffprobe_output.stderr));
            eprintln!("{}", err_msg);
            return Err(Box::new(TranscodeError(err_msg)));
        }

        let duration_str = String::from_utf8(ffprobe_output.stdout)?.trim().to_string();
        let actual_duration = duration_str.parse::<f64>()?;
        println!("Last segment ({}) or segment with non-positive calculated duration: ffprobed duration: {}", segment_index, actual_duration);
        Ok(actual_duration)
    } else {
        Ok(actual_calculated_duration_f64)
    }
}

#[cfg(test)]
mod tests {
    // Common imports and helpers for all tests in this module
    use std::path::{Path, PathBuf};
    use std::process::{Output, ExitStatus};
    use std::os::unix::process::ExitStatusExt;
    use crate::av::av::AV;
    use super::TranscodeError; // Assuming TranscodeError is defined in the parent module
    use tempfile::NamedTempFile;
    use std::io; // Ensure io is imported for ErrorKind and io::Result

    fn create_mock_std_output(stdout_str: &str, stderr_str: &str, success: bool) -> std::io::Result<Output> {
        Ok(Output {
            status: if success { ExitStatus::from_raw(0) } else { ExitStatus::from_raw(1) },
            stdout: stdout_str.as_bytes().to_vec(),
            stderr: stderr_str.as_bytes().to_vec(),
        })
    }
    
    const TEST_MEDIA_PATH_STR: &str = "/tmp/fake_media_for_test.mp4";
    fn create_test_av(segments: Vec<f64>) -> AV<'static> {
        AV {
            path: Path::new(TEST_MEDIA_PATH_STR),
            video_streams: vec![], 
            audio_streams: vec![], 
            segments,
        }
    }

    // Submodule for synchronous tests (for get_segments)
    #[cfg(test)]
    mod sync_tests {
        use super::*; 
        use crate::av::segments::get_segments;
        use crate::av::cmd::MockFfprobeRunner; // Updated path
        // std::io is inherited via super::*

        #[test]
        fn test_get_segments_valid_output() {
            let mut mock_runner = MockFfprobeRunner::new(); 
            let stdout_data = "pts_time=0.123\n[FRAME]\npts_time=1.456\n[/FRAME]\npts_time=2.789".to_string();
            
            mock_runner.expect_run_ffprobe_for_segments()
                .times(1)
                .returning(move |_| create_mock_std_output(&stdout_data, "", true)); 

            let path = PathBuf::from("dummy.mp4");
            let segments = get_segments(&path, &mock_runner); 
            assert_eq!(segments, vec![0.123, 1.456, 2.789]);
        }

        #[test]
        fn test_get_segments_no_keyframes() {
            let mut mock_runner = MockFfprobeRunner::new(); 
            mock_runner.expect_run_ffprobe_for_segments()
                .times(1)
                .returning(move |_| create_mock_std_output("", "", true));

            let path = PathBuf::from("dummy.mp4");
            let segments = get_segments(&path, &mock_runner);
            assert!(segments.is_empty());
        }

        #[test]
        fn test_get_segments_malformed_output() {
            let mut mock_runner = MockFfprobeRunner::new(); 
            let stdout_data = "pts_time=abc\npts_time=1.0\ninvalid_line\n[FRAME]\npts_time=not_a_float.but_numeric\n[/FRAME]".to_string();
            
            mock_runner.expect_run_ffprobe_for_segments()
                .times(1)
                .returning(move |_| create_mock_std_output(&stdout_data, "", true));

            let path = PathBuf::from("dummy.mp4");
            let segments = get_segments(&path, &mock_runner);
            assert_eq!(segments, vec![1.0]);
        }

        #[test]
        fn test_get_segments_ffprobe_command_fails_io_error() {
            let mut mock_runner = MockFfprobeRunner::new(); 
            mock_runner.expect_run_ffprobe_for_segments()
                .times(1)
                .returning(|_| Err(io::Error::new(io::ErrorKind::NotFound, "ffprobe not found")));

            let path = PathBuf::from("dummy.mp4");
            let segments = get_segments(&path, &mock_runner);
            assert!(segments.is_empty());
        }

        #[test]
        fn test_get_segments_ffprobe_command_fails_status_error() {
            let mut mock_runner = MockFfprobeRunner::new(); 
            mock_runner.expect_run_ffprobe_for_segments()
                .times(1)
                .returning(move |_| create_mock_std_output("", "ffprobe failed miserably", false));
                
            let path = PathBuf::from("dummy.mp4");
            let segments = get_segments(&path, &mock_runner);
            assert!(segments.is_empty());
        }
    }

    // Submodule for asynchronous tests (for transcode_at)
    #[cfg(test)]
    mod async_transcode_tests { 
        use super::*; 
        use crate::av::segments::transcode_at;
        use crate::av::cmd::MockTranscodeExecutor; // Updated path
        // use tokio::test; // Removed redundant import, #[tokio::test] is used on functions

        #[tokio::test]
        async fn test_transcode_middle_segment_success_async() { 
            let mut mock_runner = MockTranscodeExecutor::new(); 
            let av = create_test_av(vec![0.0, 10.0, 20.0]);
            let temp_file = NamedTempFile::new().unwrap();
            let at_path = temp_file.path().to_path_buf();

            mock_runner.expect_run_ffmpeg_transcode()
                .withf(move |path, start, duration, out_path| {
                    path == Path::new(TEST_MEDIA_PATH_STR) && start == "0" && duration.as_deref() == Some("10") && out_path == &at_path
                })
                .times(1)
                .returning(|_, _, _, _| {
                    let output = create_mock_std_output("ffmpeg success", "", true).unwrap();
                    Box::pin(async move { Ok(output) })
                });
        
        // No ffprobe_for_duration should be called
        mock_runner.expect_run_ffprobe_for_duration().times(0);

        let result = transcode_at(&av, 0, temp_file.path().to_path_buf(), &mock_runner).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10.0);
    }

    #[tokio::test]
    async fn test_transcode_last_segment_success() {
        let mut mock_runner = MockTranscodeExecutor::new(); // Corrected mock name
        let av = create_test_av(vec![0.0, 10.0]);
        let temp_file = NamedTempFile::new().unwrap();
        let at_path_clone1 = temp_file.path().to_path_buf();
        let at_path_clone2 = temp_file.path().to_path_buf();


        mock_runner.expect_run_ffmpeg_transcode()
            .withf(move |_, start, duration, out_path| {
                start == "10" && duration.is_none() && out_path == &at_path_clone1
            })
            .times(1)
            .returning(|_,_,_,_| {
                let output = create_mock_std_output("ffmpeg success", "", true).unwrap();
                Box::pin(async move { Ok(output) })
            });

        mock_runner.expect_run_ffprobe_for_duration()
             .withf(move |p| p == &at_path_clone2)
            .times(1)
            .returning(|_| {
                let output = create_mock_std_output("5.5\n", "", true).unwrap();
                Box::pin(async move { Ok(output) })
            });
        
        let result = transcode_at(&av, 1, temp_file.path().to_path_buf(), &mock_runner).await;
        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        assert_eq!(result.unwrap(), 5.5);
    }
    
    #[tokio::test]
    async fn test_transcode_segment_zero_duration_uses_ffprobe() {
        let mut mock_runner = MockTranscodeExecutor::new(); // Corrected mock name
        let av = create_test_av(vec![0.0, 10.0, 10.0, 20.0]);
        let temp_file = NamedTempFile::new().unwrap();
        let at_path_clone1 = temp_file.path().to_path_buf();
        let at_path_clone2 = temp_file.path().to_path_buf();

        // Expect ffmpeg to be called without -t because calculated duration is 0
        mock_runner.expect_run_ffmpeg_transcode()
            .withf(move |_, start, duration, out_path| {
                start == "10" && duration.is_none() && out_path == &at_path_clone1
            })
            .times(1)
            .returning(|_,_,_,_| {
                let output = create_mock_std_output("ffmpeg success", "", true).unwrap();
                Box::pin(async move { Ok(output) })
            });

        // Expect ffprobe for duration because calculated duration was <= 0
        mock_runner.expect_run_ffprobe_for_duration()
            .withf(move |p| p == &at_path_clone2)
            .times(1)
            .returning(|_| {
                let output = create_mock_std_output("8.8\n", "", true).unwrap();
                Box::pin(async move { Ok(output) })
            });
        
        let result = transcode_at(&av, 1, temp_file.path().to_path_buf(), &mock_runner).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 8.8);
    }

    #[tokio::test]
    async fn test_transcode_ffmpeg_fails() {
        let mut mock_runner = MockTranscodeExecutor::new(); // Corrected mock name
        let av = create_test_av(vec![0.0, 10.0]);
        let temp_file = NamedTempFile::new().unwrap();

        mock_runner.expect_run_ffmpeg_transcode()
            .times(1)
            .returning(|_,_,_,_| {
                let output = create_mock_std_output("", "ffmpeg error details", false).unwrap();
                Box::pin(async move { Ok(output) })
            });
        
        let result = transcode_at(&av, 0, temp_file.path().to_path_buf(), &mock_runner).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is::<TranscodeError>());
        assert!(err.to_string().contains("Error transcoding segment 0"));
        assert!(err.to_string().contains("ffmpeg error details"));
    }

    #[tokio::test]
    async fn test_transcode_ffprobe_duration_fails() {
        let mut mock_runner = MockTranscodeExecutor::new(); // Corrected mock name
        let av = create_test_av(vec![0.0, 10.0]); // Last segment test
        let temp_file = NamedTempFile::new().unwrap();
        let at_path_clone = temp_file.path().to_path_buf();


        mock_runner.expect_run_ffmpeg_transcode()
            .times(1)
            .returning(|_,_,_,_| {
                let output = create_mock_std_output("ffmpeg success", "", true).unwrap();
                Box::pin(async move { Ok(output) })
            });
        
        mock_runner.expect_run_ffprobe_for_duration()
            .withf(move |p| p == &at_path_clone)
            .times(1)
            .returning(|_| {
                let output = create_mock_std_output("", "ffprobe duration error", false).unwrap();
                Box::pin(async move { Ok(output) })
            });
            
        let result = transcode_at(&av, 1, temp_file.path().to_path_buf(), &mock_runner).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is::<TranscodeError>());
        assert!(err.to_string().contains("ffprobe error for"));
        assert!(err.to_string().contains("ffprobe duration error"));
    }

    #[tokio::test]
    async fn test_transcode_index_out_of_bounds() {
        let mock_runner = MockTranscodeExecutor::new(); // Corrected mock name
        let av = create_test_av(vec![0.0, 10.0]);
        let temp_file = NamedTempFile::new().unwrap();
        
        let result = transcode_at(&av, 5, temp_file.path().to_path_buf(), &mock_runner).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is::<TranscodeError>());
        assert!(err.to_string().contains("out of bounds"));
    }
} // Closes async_transcode_tests
} // Added back the closing brace for the main `mod tests`
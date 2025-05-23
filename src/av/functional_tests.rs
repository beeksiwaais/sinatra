#[cfg(test)]
mod functional_tests {
    use std::path::PathBuf;
    use crate::av::stream::{RealStreamProvider, StreamProvider}; // StreamProvider needed for trait methods
    use crate::av::segments::{get_segments, transcode_at};
    use crate::av::cmd::{RealFfprobeRunner, RealTranscodeExecutor};
    use crate::av::av::AV;
    use tempfile::tempdir; // For creating temporary directories for transcoded output

    #[test]
    fn test_functional_get_streams_real_file() {
        let path = PathBuf::from("tests/assets/empty.mp4"); 
        let provider = RealStreamProvider {};
        let streams = provider.provide_streams(&path);

        assert_eq!(streams.len(), 2, "Expected 2 streams from empty.mp4");
        let video_stream_data = streams.iter().find(|s| s.get("codec_type").and_then(|ct| ct.as_str()) == Some("video"));
        assert!(video_stream_data.is_some(), "No video stream found in empty.mp4");
    }

    #[test]
    fn test_functional_get_segments_real_file() {
        let path = PathBuf::from("tests/assets/empty.mp4");
        let runner = RealFfprobeRunner {};
        let segments = get_segments(&path, &runner);

        assert!(!segments.is_empty(), "Expected segments from empty.mp4");
        if !segments.is_empty() {
             assert_eq!(segments[0], 0.0, "Expected first segment at 0.0 for empty.mp4");
        }
    }

    #[tokio::test]
    async fn test_functional_transcode_at_real_file() {
        let test_video_path = PathBuf::from("tests/assets/empty.mp4");
        
        let stream_provider = RealStreamProvider {};
        let segment_runner = RealFfprobeRunner {};
        let av_result = AV::from_path(&test_video_path, &stream_provider, &segment_runner).await;
        assert!(av_result.is_ok(), "Failed to create AV object for transcoding test: {:?}", av_result.err());
        let av = av_result.unwrap();

        assert!(!av.segments.is_empty(), "AV object has no segments, cannot test transcode_at effectively.");
        if av.segments.is_empty() { return; } 

        let segment_index_to_transcode = 0; 
        let temp_dir = tempdir().expect("Failed to create temp dir for transcode output");
        let output_transcoded_path = temp_dir.path().join("transcoded_segment.mp4");
        
        let transcode_runner = RealTranscodeExecutor {};

        let result = transcode_at(&av, segment_index_to_transcode, output_transcoded_path.clone(), &transcode_runner).await;
        
        assert!(result.is_ok(), "transcode_at failed: {:?}", result.err());
        let transcoded_duration = result.unwrap();

        assert!(output_transcoded_path.exists(), "Transcoded output file was not created.");
        assert!(output_transcoded_path.metadata().unwrap().len() > 0, "Transcoded output file is empty.");
        assert!(transcoded_duration > 0.0, "Transcoded duration should be greater than 0. Expected segment pts: {:?}, Got duration: {:?}", av.segments.get(0), transcoded_duration);
    }
}

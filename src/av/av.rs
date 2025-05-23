use serde_json::{Result, Value}; 
use std::path::{Path, PathBuf};
use crate::av::stream::{FromStream, StreamProvider}; // Removed RealStreamProvider
use crate::av::audio_stream::AudioStream;
use crate::av::video_stream::VideoStream;
// use crate::av::stream::get_streams; // Old function, will be replaced by provider
use crate::av::segments::get_segments;
use crate::av::cmd::FfprobeRunner; // Ensured FfprobeRunner is imported (RealFfprobeRunner for default)

// #[cfg(test)]
// use mockall::{predicate::*, automock};

#[derive(Debug)]
pub(crate) struct AV<'a> {
    pub path: &'a Path, // AV holds a reference to a Path
    pub video_streams: Vec<Box<VideoStream>>,
    pub audio_streams: Vec<Box<AudioStream>>,
    pub segments: Vec<f64>,
}

impl<'a> AV<'a> { // Add lifetime 'a here
    pub async fn from_path(
        path_param: &'a PathBuf, // Input path_param has lifetime 'a
        stream_provider: &impl StreamProvider,
        segment_runner: &impl FfprobeRunner
    ) -> Result<AV<'a>> { // Return AV with lifetime 'a
        let streams_json: Vec<Value> = stream_provider.provide_streams(path_param);

        Ok(AV {
            path: path_param.as_path(), // Borrow path_param as &Path, which also has lifetime 'a
            video_streams: streams_json.iter()
                .filter_map(|stream_data| VideoStream::from_stream(stream_data))
                .collect(),
            audio_streams: streams_json.iter()
                .filter_map(|stream_data| AudioStream::from_stream(stream_data))
                .collect(),
            segments: get_segments(path_param, segment_runner),
        })
    }
}

// Assuming the previous #[cfg(test)] mod tests block might be duplicated or empty,
// this ensures we only have one, correctly defined.
// If there was an empty one, this replaces it. If it was identical, no change.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::av::stream::MockStreamProvider;
    use crate::av::cmd::MockFfprobeRunner;
    use std::path::{Path, PathBuf};
    use serde_json::json;
    // AudioStream and VideoStream are already in scope via super::*;
    // FromStream is also in scope via super::*;

    fn mock_ffprobe_output_success(stdout_str: String) -> std::io::Result<std::process::Output> {
        Ok(std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(0), // success
            stdout: stdout_str.into_bytes(),
            stderr: Vec::new(),
        })
    }

    static TEST_AV_PATH_STR: &str = "dummy_av.mp4";

    #[tokio::test]
    async fn test_from_path_no_streams_no_segments() {
        let mut mock_stream_provider = MockStreamProvider::new();
        let mut mock_segment_runner = MockFfprobeRunner::new();
        let path_buf = PathBuf::from(TEST_AV_PATH_STR);

        let path_buf_clone1 = path_buf.clone();
        mock_stream_provider.expect_provide_streams()
            .withf(move |p| p == &path_buf_clone1)
            .times(1)
            .returning(|_| vec![]);
        
        let path_buf_clone2 = path_buf.clone();
        mock_segment_runner.expect_run_ffprobe_for_segments()
            .withf(move |p| p == &path_buf_clone2)
            .times(1)
            .returning(|_| mock_ffprobe_output_success("".to_string()));

        let av_result = AV::from_path(&path_buf, &mock_stream_provider, &mock_segment_runner).await;
        assert!(av_result.is_ok());
        let av = av_result.unwrap();

        assert!(av.video_streams.is_empty());
        assert!(av.audio_streams.is_empty());
        assert!(av.segments.is_empty());
        assert_eq!(av.path, Path::new(TEST_AV_PATH_STR));
    }

    #[tokio::test]
    async fn test_from_path_video_only_with_segments() {
        let mut mock_stream_provider = MockStreamProvider::new();
        let mut mock_segment_runner = MockFfprobeRunner::new();
        let path_buf = PathBuf::from(TEST_AV_PATH_STR);

        // This JSON will be used by VideoStream::from_stream
        let video_stream_data = json!({
            "codec_type": "av", // Corrected type for VideoStream
            "codec_name": "h264", "profile": "High", "pix_fmt": "yuv420p", 
            "color_space": "bt709", "frame_rate": "30/1", "bit_rate": "5000k",
            "width": 1920, "height": 1080, "aspect_ratio": "16:9" 
        });

        let path_buf_clone1 = path_buf.clone();
        mock_stream_provider.expect_provide_streams()
            .withf(move |p| p == &path_buf_clone1) // Added withf for path_buf check
            .times(1)
            .returning(move |_| vec![video_stream_data.clone()]);
        
        let segments_stdout = "[FRAME]\npts_time=1.0\n[/FRAME]\n[FRAME]\npts_time=2.0\n[/FRAME]".to_string();
        let path_buf_clone2 = path_buf.clone();
        mock_segment_runner.expect_run_ffprobe_for_segments()
            .withf(move |p| p == &path_buf_clone2) // Added withf for path_buf check
            .times(1)
            .returning(move |_| mock_ffprobe_output_success(segments_stdout.clone()));

        let av_result = AV::from_path(&path_buf, &mock_stream_provider, &mock_segment_runner).await;
        assert!(av_result.is_ok());
        let av = av_result.unwrap();

        assert_eq!(av.video_streams.len(), 1);
        assert!(av.audio_streams.is_empty());
        assert_eq!(av.segments, vec![1.0, 2.0]);
    }

    #[tokio::test]
    async fn test_from_path_audio_only_with_segments() {
        let mut mock_stream_provider = MockStreamProvider::new();
        let mut mock_segment_runner = MockFfprobeRunner::new();
        let path_buf = PathBuf::from(TEST_AV_PATH_STR);

        let audio_stream_data = json!({
            "codec_type": "audio", 
            "codec_name": "aac", "profile": "LC", "bit_rate": "128k"
            // Assuming AudioStream::from_stream uses these fields
        });

        let path_buf_clone1 = path_buf.clone();
        mock_stream_provider.expect_provide_streams()
            .withf(move |p| p == &path_buf_clone1) // Added withf for path_buf check
            .times(1)
            .returning(move |_| vec![audio_stream_data.clone()]);
        
        let segments_stdout = "[FRAME]\npts_time=3.5\n[/FRAME]\n[FRAME]\npts_time=4.5\n[/FRAME]".to_string();
        let path_buf_clone2 = path_buf.clone();
        mock_segment_runner.expect_run_ffprobe_for_segments()
            .withf(move |p| p == &path_buf_clone2) // Added withf for path_buf check
            .times(1)
            .returning(move |_| mock_ffprobe_output_success(segments_stdout.clone()));

        let av_result = AV::from_path(&path_buf, &mock_stream_provider, &mock_segment_runner).await;
        assert!(av_result.is_ok());
        let av = av_result.unwrap();

        assert!(av.video_streams.is_empty());
        assert_eq!(av.audio_streams.len(), 1);
        assert_eq!(av.segments, vec![3.5, 4.5]);
    }

    #[tokio::test]
    async fn test_from_path_mixed_streams_and_segments() {
        let mut mock_stream_provider = MockStreamProvider::new();
        let mut mock_segment_runner = MockFfprobeRunner::new();
        let path_buf = PathBuf::from(TEST_AV_PATH_STR);

        let video_stream_data = json!({
            "codec_type": "av", "codec_name": "h264", "profile": "High", "width": 1920, "height": 1080,
            "pix_fmt": "yuv420p", "color_space": "bt709", "frame_rate": "25/1", "bit_rate": "3000k", "aspect_ratio": "16:9"
        });
        let audio_stream_data = json!({
            "codec_type": "audio", "codec_name": "mp3", "profile": "N/A", "bit_rate": "192k"
        });

        let path_buf_clone1 = path_buf.clone();
        mock_stream_provider.expect_provide_streams()
            .withf(move |p| p == &path_buf_clone1) // Added withf for path_buf check
            .times(1)
            .returning(move |_| vec![video_stream_data.clone(), audio_stream_data.clone()]);
        
        let segments_stdout = "[FRAME]\npts_time=0.1\n[/FRAME]".to_string();
        let path_buf_clone2 = path_buf.clone();
        mock_segment_runner.expect_run_ffprobe_for_segments()
            .withf(move |p| p == &path_buf_clone2) // Added withf for path_buf check
            .times(1)
            .returning(move |_| mock_ffprobe_output_success(segments_stdout.clone()));

        let av_result = AV::from_path(&path_buf, &mock_stream_provider, &mock_segment_runner).await;
        assert!(av_result.is_ok());
        let av = av_result.unwrap();

        assert_eq!(av.video_streams.len(), 1);
        assert_eq!(av.audio_streams.len(), 1);
        assert_eq!(av.segments, vec![0.1]);
    }

    #[tokio::test]
    async fn test_from_path_filters_invalid_stream_data() {
        let mut mock_stream_provider = MockStreamProvider::new();
        let mut mock_segment_runner = MockFfprobeRunner::new();
        let path_buf = PathBuf::from(TEST_AV_PATH_STR);

        // Video stream missing "codec_name" which is required by VideoStream::from_stream if it uses `?`
        let invalid_video_stream_data = json!({
            "codec_type": "av", "profile": "High", "width": 1920, "height": 1080,
            "pix_fmt": "yuv420p", "color_space": "bt709", "frame_rate": "25/1", "bit_rate": "3000k", "aspect_ratio": "16:9"
        });
         let valid_audio_stream_data = json!({
            "codec_type": "audio", "codec_name": "aac", "profile": "LC", "bit_rate": "128k"
        });

        let path_buf_clone1 = path_buf.clone();
        mock_stream_provider.expect_provide_streams()
            .withf(move |p| p == &path_buf_clone1) // Added withf for path_buf check
            .times(1)
            .returning(move |_| vec![invalid_video_stream_data.clone(), valid_audio_stream_data.clone()]);
        
        let path_buf_clone2 = path_buf.clone();
        mock_segment_runner.expect_run_ffprobe_for_segments()
            .withf(move |p| p == &path_buf_clone2) // Added withf for path_buf check
            .times(1)
            .returning(|_| mock_ffprobe_output_success("".to_string()));

        let av_result = AV::from_path(&path_buf, &mock_stream_provider, &mock_segment_runner).await;
        assert!(av_result.is_ok());
        let av = av_result.unwrap();

        assert!(av.video_streams.is_empty(), "Video stream should have been filtered out");
        assert_eq!(av.audio_streams.len(), 1, "Audio stream should still be present");
        assert!(av.segments.is_empty());
    }
}

// Removed the duplicated tests block that started here.
// The first `mod tests` block (lines 46-188 in the read_files output) is the one to keep.
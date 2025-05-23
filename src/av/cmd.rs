use std::path::{Path, PathBuf};
use std::process::Output;
use std::io;
use async_trait::async_trait;
use tokio::process::Command as TokioCommand;

// Trait for synchronous operations like get_segments (acting as FfprobeRunner)
#[cfg_attr(test, mockall::automock)] 
pub trait FfprobeRunner { 
    fn run_ffprobe_for_segments(&self, path: &PathBuf) -> io::Result<Output>;
}

pub struct RealFfprobeRunner; 

impl FfprobeRunner for RealFfprobeRunner { 
    fn run_ffprobe_for_segments(&self, path: &PathBuf) -> io::Result<Output> {
        std::process::Command::new("ffprobe")
            .arg("-select_streams").arg("v")
            .arg("-skip_frame").arg("nokey")
            .arg("-show_frames").arg("-v").arg("quiet")
            .arg(path)
            .output()
    }
}

// New trait specifically for transcoding operations
#[async_trait]
#[cfg_attr(test, mockall::automock)] 
pub trait TranscodeExecutor {
    async fn run_ffmpeg_transcode(&self, av_path: &Path, start_at: &str, duration: Option<String>, output_path: &PathBuf) -> io::Result<Output>;
    async fn run_ffprobe_for_duration(&self, media_path: &PathBuf) -> io::Result<Output>;
}

pub struct RealTranscodeExecutor;

#[async_trait]
impl TranscodeExecutor for RealTranscodeExecutor {
    async fn run_ffmpeg_transcode(&self, av_path: &Path, start_at: &str, duration: Option<String>, output_path: &PathBuf) -> io::Result<Output> {
        let mut command = TokioCommand::new("ffmpeg");
        command.arg("-y").arg("-ss").arg(start_at).arg("-i").arg(av_path);
        if let Some(dur) = duration {
            command.arg("-t").arg(dur);
        }
        command.arg(output_path);
        command.output().await
    }

    async fn run_ffprobe_for_duration(&self, media_path: &PathBuf) -> io::Result<Output> {
        TokioCommand::new("ffprobe")
            .arg("-v").arg("error")
            .arg("-show_entries").arg("format=duration")
            .arg("-of").arg("default=noprint_wrappers=1:nokey=1")
            .arg(media_path)
            .output()
            .await
    }
}

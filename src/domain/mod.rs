//! Domain layer - Pure business logic.

// Video analysis modules (require ffmpeg-next)
#[cfg(any(
    feature = "local",
    feature = "aws_orchestrator",
    feature = "aws_worker"
))]
pub mod av;

// HLS generation (only local + worker, not orchestrator)
#[cfg(any(feature = "local", feature = "aws_worker"))]
pub mod hls;

// Job definitions (always available)
pub mod jobs;

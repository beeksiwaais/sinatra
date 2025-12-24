//! Audio/Video domain modules.

pub mod audio_stream;
pub mod av;
pub mod segments;
pub mod stream;
pub mod video_stream;

// Thumbnails only needed by worker (requires image crate)
#[cfg(any(feature = "local", feature = "aws_worker"))]
pub mod thumbnails;

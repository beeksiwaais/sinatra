//! Application layer - Generic services that use ports.

// Orchestrator: needs video analysis (av module) - local, aws_orchestrator, aws_worker
#[cfg(any(
    feature = "local",
    feature = "aws_orchestrator",
    feature = "aws_worker"
))]
pub mod orchestrator;

// Worker: needs video processing + hls - only local and aws_worker
#[cfg(any(feature = "local", feature = "aws_worker"))]
pub mod worker;

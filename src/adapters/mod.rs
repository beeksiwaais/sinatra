//! Adapters - Concrete implementations of ports.

#[cfg(any(feature = "aws_orchestrator", feature = "aws_worker"))]
pub mod aws;

#[cfg(feature = "local")]
pub mod local;

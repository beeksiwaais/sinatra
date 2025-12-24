//! HTTP/S3-compatible inbound adapter.
//!
//! This module provides an S3-compatible HTTP API for external clients
//! to upload and download files.

pub mod buckets;
mod s3;

pub use s3::LocalS3;

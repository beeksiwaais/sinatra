//! Local adapters for monolith deployment.

pub mod events;
pub mod fs;
pub mod http;
pub mod redis;

pub use events::hub::EventHub;
pub use http::{buckets, LocalS3};
pub use redis::{RedisPool, RedisQueue};

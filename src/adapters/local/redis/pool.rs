//! Redis connection pool.

use super::error::QueueError;
use deadpool_redis::{Config, Pool, Runtime};

/// Redis-backed adapter for queue and repository operations.
#[derive(Clone)]
pub struct RedisPool {
    pub(super) pool: Pool,
}

impl RedisPool {
    /// Create a new RedisPool with connection pool.
    pub fn new(redis_url: &str) -> Result<Self, QueueError> {
        let cfg = Config::from_url(redis_url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        Ok(Self { pool })
    }
}

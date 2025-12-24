//! Redis error types for the local adapter.

use deadpool_redis::CreatePoolError;
use std::fmt;

pub type RedisError = deadpool_redis::redis::RedisError;
pub type PoolError = deadpool_redis::PoolError;

#[derive(Debug)]
pub enum QueueError {
    Redis(RedisError),
    Pool(PoolError),
    Serialization(serde_json::Error),
    CreatePool(String),
}

impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueueError::Redis(e) => write!(f, "Redis error: {}", e),
            QueueError::Pool(e) => write!(f, "Pool error: {}", e),
            QueueError::Serialization(e) => write!(f, "Serialization error: {}", e),
            QueueError::CreatePool(e) => write!(f, "Create pool error: {}", e),
        }
    }
}

impl std::error::Error for QueueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            QueueError::Redis(e) => Some(e),
            QueueError::Pool(e) => Some(e),
            QueueError::Serialization(e) => Some(e),
            QueueError::CreatePool(_) => None,
        }
    }
}

impl From<RedisError> for QueueError {
    fn from(err: RedisError) -> Self {
        QueueError::Redis(err)
    }
}

impl From<PoolError> for QueueError {
    fn from(err: PoolError) -> Self {
        QueueError::Pool(err)
    }
}

impl From<serde_json::Error> for QueueError {
    fn from(err: serde_json::Error) -> Self {
        QueueError::Serialization(err)
    }
}

impl From<CreatePoolError> for QueueError {
    fn from(err: CreatePoolError) -> Self {
        QueueError::CreatePool(format!("{}", err))
    }
}

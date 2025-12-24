//! Configuration for different deployment environments.

use std::env;

/// Configuration for local/monolith deployment.
#[cfg(feature = "local")]
#[derive(Clone, Debug)]
pub struct LocalConfig {
    /// HTTP server bind address
    pub addr: String,
    /// HTTP server port
    pub port: String,
    /// Redis connection URL
    pub redis_url: String,
    /// Directory for file uploads and HLS output
    pub upload_dir: String,
    /// AWS Access Key ID for S3-compatible API authentication
    pub aws_access_key_id: String,
    /// AWS Secret Access Key for S3-compatible API authentication
    pub aws_secret_access_key: String,
}

#[cfg(feature = "local")]
impl LocalConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        dotenv::dotenv().ok();

        Self {
            addr: env::var("ADDR").unwrap_or_else(|_| String::from("127.0.0.1")),
            port: env::var("PORT").unwrap_or_else(|_| String::from("3000")),
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| String::from("redis://127.0.0.1/")),
            upload_dir: env::var("UPLOAD_DIR").unwrap_or_else(|_| String::from("./")),
            aws_access_key_id: env::var("AWS_ACCESS_KEY_ID")
                .unwrap_or_else(|_| String::from("minioadmin")),
            aws_secret_access_key: env::var("AWS_SECRET_ACCESS_KEY")
                .unwrap_or_else(|_| String::from("minioadmin")),
        }
    }
}

/// Configuration for AWS/serverless deployment.
#[cfg(any(feature = "aws_orchestrator", feature = "aws_worker"))]
#[derive(Clone, Debug)]
pub struct AwsConfig {
    /// S3 bucket for video storage
    pub s3_bucket: String,
    /// SQS queue URL for job messages
    pub sqs_queue_url: String,
    /// DynamoDB table name for video state
    pub dynamodb_table: String,
}

#[cfg(any(feature = "aws_orchestrator", feature = "aws_worker"))]
impl AwsConfig {
    /// Load configuration from environment variables.
    /// Panics if required variables are not set.
    pub fn from_env() -> Self {
        Self {
            s3_bucket: env::var("S3_BUCKET").expect("S3_BUCKET env var required"),
            sqs_queue_url: env::var("SQS_QUEUE_URL").expect("SQS_QUEUE_URL env var required"),
            dynamodb_table: env::var("DYNAMODB_TABLE").expect("DYNAMODB_TABLE env var required"),
        }
    }
}

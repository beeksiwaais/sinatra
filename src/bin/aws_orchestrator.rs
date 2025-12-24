//! AWS Orchestrator Binary
//!
//! This binary is intended to be deployed as an AWS Lambda function triggered by S3 events.
//! It receives notifications of new video uploads and enqueues processing jobs.
//!
//! Environment Variables:
//! - AWS_REGION: AWS region
//! - S3_BUCKET: S3 bucket for video storage
//! - SQS_QUEUE_URL: SQS queue URL for jobs
//! - DYNAMODB_TABLE: DynamoDB table for video state

use sinatra::adapters::aws::{dynamodb::DynamoAdapter, s3::S3Adapter, sqs::SqsAdapter};
use sinatra::application::orchestrator::OrchestratorService;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Load AWS config
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;

    // Environment variables
    let bucket = std::env::var("S3_BUCKET").expect("S3_BUCKET env var required");
    let queue_url = std::env::var("SQS_QUEUE_URL").expect("SQS_QUEUE_URL env var required");
    let table_name = std::env::var("DYNAMODB_TABLE").expect("DYNAMODB_TABLE env var required");

    // Create AWS clients
    let s3_client = aws_sdk_s3::Client::new(&config);
    let sqs_client = aws_sdk_sqs::Client::new(&config);
    let dynamo_client = aws_sdk_dynamodb::Client::new(&config);

    // Create adapters
    let storage = S3Adapter::new(s3_client, bucket);
    let queue = SqsAdapter::new(sqs_client, queue_url);
    let repo = DynamoAdapter::new(dynamo_client, table_name);

    // Create Orchestrator service
    let orchestrator = Arc::new(OrchestratorService::new(storage, queue, repo));

    // In Lambda context, this would be triggered by S3 event.
    // For now, read video key from environment or stdin for testing.
    let video_key = std::env::var("VIDEO_KEY").unwrap_or_else(|_| {
        eprintln!("VIDEO_KEY env var not set. In Lambda, this comes from S3 event.");
        std::process::exit(1);
    });

    println!("Processing new video: {}", video_key);

    match orchestrator.handle_new_video(&video_key).await {
        Ok(video_id) => println!("Successfully enqueued video: {}", video_id),
        Err(e) => eprintln!("Failed to process video: {:?}", e),
    }
}

//! AWS Worker Binary
//!
//! This binary is intended to be deployed as an AWS Lambda function (or Batch job) that:
//! 1. Connects to AWS services (S3, SQS, DynamoDB).
//! 2. Runs the WorkerService in a loop to process video transcoding jobs.
//!
//! Environment Variables:
//! - AWS_REGION: AWS region (e.g., us-east-1)
//! - S3_BUCKET: S3 bucket for video storage
//! - SQS_QUEUE_URL: SQS queue URL for jobs
//! - DYNAMODB_TABLE: DynamoDB table for video state

use sinatra::adapters::aws::{dynamodb::DynamoAdapter, s3::S3Adapter, sqs::SqsAdapter};
use sinatra::application::worker::WorkerService;
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

    // Create and run Worker service
    let worker = Arc::new(WorkerService::new(storage, queue, repo));

    println!("AWS Worker started, polling for jobs...");

    // Single worker loop (Lambda invokes once per trigger)
    // For Batch/long-running, could loop forever
    worker.run_worker_loop(0).await;
}

# Sinatra AWS Infrastructure

Pulumi infrastructure-as-code (Rust) for deploying Sinatra video processing to AWS.

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   S3: stream/   │────▶│   Orchestrator   │────▶│   SQS Queue     │
│   (uploads)     │     │   Lambda         │     │   (jobs)        │
└─────────────────┘     └──────────────────┘     └────────┬────────┘
                                                          │
                                                          ▼
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   CloudFront    │◀────│   S3: hls/       │◀────│   Worker        │
│   CDN           │     │   (output)       │     │   Lambda        │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                                                          │
                                                          ▼
                                                 ┌─────────────────┐
                                                 │   DynamoDB      │
                                                 │   (state)       │
                                                 └─────────────────┘
```

## Resources Created

| Resource | Name | Purpose |
|----------|------|---------|
| S3 Bucket | `sinatra-videos` | Video upload storage |
| S3 Bucket | `sinatra-hls` | HLS output (CDN origin) |
| CloudFront | CDN | Low-latency HLS streaming |
| SQS Queue | `sinatra-jobs` | Job queue for workers |
| DynamoDB | `sinatra-video-state` | Video processing state |
| Lambda | `sinatra-orchestrator` | S3 trigger → creates jobs |
| Lambda | `sinatra-worker` | SQS trigger → transcodes |
| IAM Role | `sinatra-lambda-role` | Permissions for Lambdas |

## Prerequisites

1. Install [Pulumi](https://www.pulumi.com/docs/install/)
2. Install [Rust](https://rustup.rs/)
3. Install [cargo-lambda](https://www.cargo-lambda.info/guide/installation.html)
4. Configure AWS credentials: `aws configure`

## Building Lambda Binaries

```bash
# From project root
cd ..

# Build for Lambda (x86_64)
cargo lambda build --release --bin aws_worker --bin aws_orchestrator

# Or for ARM64 (Graviton2 - cheaper)
cargo lambda build --release --arm64 --bin aws_worker --bin aws_orchestrator

# Package for deployment
cd target/lambda
zip aws_worker.zip aws_worker/bootstrap
zip aws_orchestrator.zip aws_orchestrator/bootstrap

# Upload to deploy bucket
aws s3 cp aws_worker.zip s3://sinatra-deploy/lambda/
aws s3 cp aws_orchestrator.zip s3://sinatra-deploy/lambda/
```

## Deploying Infrastructure

```bash
cd infra

# Build Pulumi program
cargo build

# Create stack
pulumi stack init dev

# Set AWS region
pulumi config set aws:region us-east-1

# Preview changes
pulumi preview

# Deploy
pulumi up

# View outputs
pulumi stack output
```

## Outputs

| Output | Description |
|--------|-------------|
| `videos_bucket_name` | S3 bucket for uploads |
| `hls_bucket_name` | S3 bucket for HLS output |
| `hls_cdn_url` | CloudFront URL for streaming |
| `job_queue_url` | SQS queue URL |
| `video_state_table_name` | DynamoDB table |
| `orchestrator_lambda_arn` | Orchestrator Lambda ARN |
| `worker_lambda_arn` | Worker Lambda ARN |

## Uploading Videos

```bash
# Get bucket name
BUCKET=$(pulumi stack output videos_bucket_name)

# Upload video (triggers orchestrator)
aws s3 cp my_video.mp4 s3://$BUCKET/stream/my_video.mp4

# Watch logs
aws logs tail /aws/lambda/sinatra-orchestrator --follow
aws logs tail /aws/lambda/sinatra-worker --follow
```

## Cleanup

```bash
pulumi destroy
```

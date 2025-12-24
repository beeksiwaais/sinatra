//! Sinatra AWS Infrastructure
//!
//! Pulumi infrastructure-as-code for deploying:
//! - S3 bucket for video storage
//! - S3 bucket + CloudFront CDN for HLS streaming
//! - SQS queue for job processing
//! - DynamoDB table for video state
//! - Lambda functions for orchestrator and worker

use pulumi_wasm::pulumi_main;
use pulumi_wasm_aws::cloudfront;
use pulumi_wasm_aws::dynamodb;
use pulumi_wasm_aws::iam;
use pulumi_wasm_aws::lambda;
use pulumi_wasm_aws::s3;
use pulumi_wasm_aws::sqs;

#[pulumi_main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ==========================================================================
    // S3 BUCKETS
    // ==========================================================================

    // S3 Bucket for video uploads (private)
    let videos_bucket = s3::Bucket::builder()
        .bucket("sinatra-videos")
        .tags(vec![("Project", "sinatra"), ("Environment", "production")])
        .build()?;

    // CORS configuration for presigned URL uploads from browsers
    let cors_origins: Vec<String> = pulumi_wasm::Config::get("cors_allowed_origins")
        .map(|s| s.split(',').map(String::from).collect())
        .unwrap_or_else(|| vec!["*".to_string()]);

    let _videos_cors = s3::BucketCorsConfigurationV2::builder()
        .bucket(videos_bucket.id.clone())
        .cors_rules(vec![s3::BucketCorsConfigurationV2CorsRuleArgs {
            allowed_headers: vec!["*".to_string()],
            allowed_methods: vec!["PUT".to_string(), "POST".to_string()],
            allowed_origins: cors_origins,
            expose_headers: vec!["ETag".to_string()],
            max_age_seconds: Some(3600),
            ..Default::default()
        }])
        .build()?;

    // S3 Bucket for HLS output (CloudFront origin)
    let hls_bucket = s3::Bucket::builder().bucket("sinatra-hls").build()?;

    // ==========================================================================
    // CLOUDFRONT CDN
    // ==========================================================================

    let oac = cloudfront::OriginAccessControl::builder()
        .name("sinatra-hls-oac")
        .origin_access_control_origin_type("s3")
        .signing_behavior("always")
        .signing_protocol("sigv4")
        .build()?;

    let cdn = cloudfront::Distribution::builder()
        .enabled(true)
        .comment("Sinatra HLS CDN")
        .default_root_object("index.html")
        .origins(vec![cloudfront::DistributionOriginArgs {
            domain_name: hls_bucket.bucket_regional_domain_name.clone(),
            origin_id: "hls-bucket".to_string(),
            origin_access_control_id: Some(oac.id.clone()),
            s3_origin_config: None,
        }])
        .default_cache_behavior(cloudfront::DistributionDefaultCacheBehaviorArgs {
            target_origin_id: "hls-bucket".to_string(),
            viewer_protocol_policy: "redirect-to-https".to_string(),
            allowed_methods: vec!["GET", "HEAD", "OPTIONS"],
            cached_methods: vec!["GET", "HEAD"],
            compress: true,
            min_ttl: 0,
            default_ttl: 86400,
            max_ttl: 31536000,
            forwarded_values: cloudfront::DistributionDefaultCacheBehaviorForwardedValuesArgs {
                query_string: false,
                cookies: cloudfront::DistributionDefaultCacheBehaviorForwardedValuesCookiesArgs {
                    forward: "none".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
        })
        .restrictions(cloudfront::DistributionRestrictionsArgs {
            geo_restriction: cloudfront::DistributionRestrictionsGeoRestrictionArgs {
                restriction_type: "none".to_string(),
                locations: vec![],
            },
        })
        .viewer_certificate(cloudfront::DistributionViewerCertificateArgs {
            cloudfront_default_certificate: true,
            ..Default::default()
        })
        .build()?;

    // Bucket policy for CloudFront
    let _hls_bucket_policy = s3::BucketPolicy::builder()
        .bucket(hls_bucket.id.clone())
        .policy(format!(
            r#"{{
            "Version": "2012-10-17",
            "Statement": [{{
                "Sid": "AllowCloudFrontServicePrincipal",
                "Effect": "Allow",
                "Principal": {{"Service": "cloudfront.amazonaws.com"}},
                "Action": "s3:GetObject",
                "Resource": "arn:aws:s3:::sinatra-hls/*",
                "Condition": {{"StringEquals": {{"AWS:SourceArn": "{}"}}}}
            }}]
        }}"#,
            cdn.arn
        ))
        .build()?;

    // ==========================================================================
    // SQS QUEUE
    // ==========================================================================

    let job_queue = sqs::Queue::builder()
        .name("sinatra-jobs")
        .visibility_timeout_seconds(300)
        .message_retention_seconds(86400)
        .build()?;

    // ==========================================================================
    // DYNAMODB TABLE
    // ==========================================================================

    let video_state_table = dynamodb::Table::builder()
        .name("sinatra-video-state")
        .billing_mode("PAY_PER_REQUEST")
        .hash_key("video_id")
        .attributes(vec![dynamodb::TableAttributeArgs {
            name: "video_id".to_string(),
            r#type: "S".to_string(),
        }])
        .build()?;

    // ==========================================================================
    // IAM ROLE FOR LAMBDA
    // ==========================================================================

    let lambda_assume_role_policy = r#"{
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": {"Service": "lambda.amazonaws.com"},
            "Action": "sts:AssumeRole"
        }]
    }"#;

    let lambda_role = iam::Role::builder()
        .name("sinatra-lambda-role")
        .assume_role_policy(lambda_assume_role_policy.to_string())
        .build()?;

    // Attach policies for S3, SQS, DynamoDB, CloudWatch Logs
    let lambda_policy = iam::RolePolicy::builder()
        .name("sinatra-lambda-policy")
        .role(lambda_role.id.clone())
        .policy(format!(r#"{{
            "Version": "2012-10-17",
            "Statement": [
                {{
                    "Effect": "Allow",
                    "Action": ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"],
                    "Resource": ["{}/*", "{}/*"]
                }},
                {{
                    "Effect": "Allow",
                    "Action": ["sqs:SendMessage", "sqs:ReceiveMessage", "sqs:DeleteMessage", "sqs:GetQueueAttributes"],
                    "Resource": "{}"
                }},
                {{
                    "Effect": "Allow",
                    "Action": ["dynamodb:GetItem", "dynamodb:PutItem", "dynamodb:UpdateItem", "dynamodb:DeleteItem"],
                    "Resource": "{}"
                }},
                {{
                    "Effect": "Allow",
                    "Action": ["logs:CreateLogGroup", "logs:CreateLogStream", "logs:PutLogEvents"],
                    "Resource": "arn:aws:logs:*:*:*"
                }}
            ]
        }}"#, videos_bucket.arn, hls_bucket.arn, job_queue.arn, video_state_table.arn))
        .build()?;

    // ==========================================================================
    // LAMBDA FUNCTIONS
    // ==========================================================================

    // Orchestrator Lambda - triggered by S3 uploads
    let orchestrator_lambda = lambda::Function::builder()
        .function_name("sinatra-orchestrator")
        .runtime("provided.al2") // Custom runtime for Rust
        .handler("bootstrap") // Rust binary name
        .role(lambda_role.arn.clone())
        .timeout(30)
        .memory_size(256)
        .code(lambda::FunctionCodeArgs {
            s3_bucket: Some("sinatra-deploy".to_string()), // Deploy bucket for Lambda code
            s3_key: Some("lambda/aws_orchestrator.zip".to_string()),
            ..Default::default()
        })
        .environment(lambda::FunctionEnvironmentArgs {
            variables: vec![
                ("S3_BUCKET".to_string(), videos_bucket.bucket.clone()),
                ("SQS_QUEUE_URL".to_string(), job_queue.url.clone()),
                ("DYNAMODB_TABLE".to_string(), video_state_table.name.clone()),
                ("HLS_BUCKET".to_string(), hls_bucket.bucket.clone()),
            ]
            .into_iter()
            .collect(),
        })
        .build()?;

    // Worker Lambda - triggered by SQS messages
    let worker_lambda = lambda::Function::builder()
        .function_name("sinatra-worker")
        .runtime("provided.al2")
        .handler("bootstrap")
        .role(lambda_role.arn.clone())
        .timeout(300) // 5 minutes for transcoding
        .memory_size(1024) // More memory for FFmpeg
        .code(lambda::FunctionCodeArgs {
            s3_bucket: Some("sinatra-deploy".to_string()),
            s3_key: Some("lambda/aws_worker.zip".to_string()),
            ..Default::default()
        })
        .environment(lambda::FunctionEnvironmentArgs {
            variables: vec![
                ("S3_BUCKET".to_string(), videos_bucket.bucket.clone()),
                ("SQS_QUEUE_URL".to_string(), job_queue.url.clone()),
                ("DYNAMODB_TABLE".to_string(), video_state_table.name.clone()),
                ("HLS_BUCKET".to_string(), hls_bucket.bucket.clone()),
            ]
            .into_iter()
            .collect(),
        })
        .build()?;

    // ==========================================================================
    // EVENT TRIGGERS
    // ==========================================================================

    // S3 -> Orchestrator trigger
    let _s3_permission = lambda::Permission::builder()
        .function_name(orchestrator_lambda.function_name.clone())
        .action("lambda:InvokeFunction")
        .principal("s3.amazonaws.com")
        .source_arn(videos_bucket.arn.clone())
        .build()?;

    let _s3_notification = s3::BucketNotification::builder()
        .bucket(videos_bucket.id.clone())
        .lambda_functions(vec![s3::BucketNotificationLambdaFunctionArgs {
            lambda_function_arn: orchestrator_lambda.arn.clone(),
            events: vec!["s3:ObjectCreated:*".to_string()],
            filter_prefix: Some("stream/".to_string()), // Only trigger on stream/ uploads
            ..Default::default()
        }])
        .build()?;

    // SQS -> Worker trigger
    let _sqs_mapping = lambda::EventSourceMapping::builder()
        .event_source_arn(job_queue.arn.clone())
        .function_name(worker_lambda.arn.clone())
        .batch_size(1) // Process one job at a time
        .build()?;

    // ==========================================================================
    // OUTPUTS
    // ==========================================================================

    pulumi_wasm::export("videos_bucket_name", &videos_bucket.bucket);
    pulumi_wasm::export("videos_bucket_arn", &videos_bucket.arn);
    pulumi_wasm::export("hls_bucket_name", &hls_bucket.bucket);
    pulumi_wasm::export("hls_cdn_domain", &cdn.domain_name);
    pulumi_wasm::export("hls_cdn_url", format!("https://{}", &cdn.domain_name));
    pulumi_wasm::export("job_queue_url", &job_queue.url);
    pulumi_wasm::export("job_queue_arn", &job_queue.arn);
    pulumi_wasm::export("video_state_table_name", &video_state_table.name);
    pulumi_wasm::export("orchestrator_lambda_arn", &orchestrator_lambda.arn);
    pulumi_wasm::export("worker_lambda_arn", &worker_lambda.arn);

    Ok(())
}

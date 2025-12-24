use super::super::LocalS3;
use futures::TryStreamExt;
use s3s::dto::*;
use s3s::{S3Error, S3ErrorCode, S3Request, S3Response, S3Result};
use tokio::fs::File;
use tokio::io::BufWriter;
use tokio_util::io::StreamReader;

pub async fn handle(
    s3: &LocalS3,
    req: S3Request<PutObjectInput>,
) -> S3Result<S3Response<PutObjectOutput>> {
    let input = req.input;
    let bucket = input.bucket;
    let key = input.key;
    let body = input.body;

    // Validate bucket using registry
    let bucket_config = super::bucket::find(&bucket).ok_or_else(|| {
        s3s::S3Error::with_message(s3s::S3ErrorCode::NoSuchBucket, "Invalid bucket.")
    })?;

    if !bucket_config.allow_put {
        return Err(s3s::S3Error::with_message(
            s3s::S3ErrorCode::MethodNotAllowed,
            "PUT not allowed for this bucket.",
        ));
    }

    if let Some(body_stream) = body {
        // Prevent directory traversal
        if key.contains("..") {
            return Err(s3s::S3Error::with_message(
                s3s::S3ErrorCode::InvalidRequest,
                "Invalid key",
            ));
        }

        // Path: upload_dir/<bucket>/<key>
        let path = s3.upload_dir.join(&bucket).join(&key);

        // Ensure parent dir exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                s3s::S3Error::with_source(s3s::S3ErrorCode::InternalError, Box::new(e))
            })?;
        }

        println!("S3: Saving file to {:?}", path);

        // Convert ByteStream to AsyncRead
        let stream = body_stream.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
        let mut body_reader = StreamReader::new(stream);

        let file = File::create(&path)
            .await
            .map_err(|e| S3Error::with_source(S3ErrorCode::InternalError, Box::new(e)))?;
        let mut file_writer = BufWriter::new(file);

        tokio::io::copy(&mut body_reader, &mut file_writer)
            .await
            .map_err(|e| S3Error::with_source(S3ErrorCode::InternalError, Box::new(e)))?;

        let metadata = input.metadata;

        if bucket_config.events_enabled {
            if let Some(factory) = bucket_config.upload_event_builder {
                let event = factory(path.clone(), bucket.clone(), metadata);
                if let Err(e) = s3.event_hub.publish(event) {
                    eprintln!("Failed to publish event: {:?}", e);
                }
            }
        }
    }

    Ok(S3Response::new(PutObjectOutput::default()))
}

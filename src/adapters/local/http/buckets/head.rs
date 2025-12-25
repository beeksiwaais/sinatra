use super::super::LocalS3;
use s3s::dto::*;
use s3s::{S3Request, S3Response, S3Result};

pub async fn handle(
    s3: &LocalS3,
    req: S3Request<HeadObjectInput>,
) -> S3Result<S3Response<HeadObjectOutput>> {
    let input = req.input;
    let bucket = input.bucket;
    let key = input.key;

    // Validate bucket using registry
    let bucket_config = super::bucket::find(&bucket).ok_or_else(|| {
        s3s::S3Error::with_message(s3s::S3ErrorCode::NoSuchBucket, "Invalid bucket.")
    })?;

    if !bucket_config.allow_get {
        return Err(s3s::S3Error::with_message(
            s3s::S3ErrorCode::MethodNotAllowed,
            "HEAD not allowed for this bucket.",
        ));
    }

    // Prevent directory traversal
    if key.contains("..") {
        return Err(s3s::S3Error::with_message(
            s3s::S3ErrorCode::InvalidRequest,
            "Invalid key",
        ));
    }

    // Path: upload_dir/<bucket>/<key>
    let path = s3.upload_dir.join(&bucket).join(&key);

    let metadata = tokio::fs::metadata(&path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            s3s::S3Error::with_message(s3s::S3ErrorCode::NoSuchKey, "Key not found")
        } else {
            s3s::S3Error::with_source(s3s::S3ErrorCode::InternalError, Box::new(e))
        }
    })?;

    // Basic mime detection
    let content_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();

    let mut output = HeadObjectOutput::default();
    output.content_length = Some(metadata.len() as i64);
    output.content_type = Some(content_type);

    Ok(S3Response::new(output))
}

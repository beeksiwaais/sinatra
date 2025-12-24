use super::super::LocalS3;
use s3s::dto::*;
use s3s::{S3Request, S3Response, S3Result};

pub async fn handle(
    s3: &LocalS3,
    req: S3Request<GetObjectInput>,
) -> S3Result<S3Response<GetObjectOutput>> {
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
            "GET not allowed for this bucket.",
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

    if !path.exists() {
        return Err(s3s::S3Error::with_message(
            s3s::S3ErrorCode::NoSuchKey,
            "Key not found",
        ));
    }

    // Using read() to load content into memory.
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| s3s::S3Error::with_source(s3s::S3ErrorCode::InternalError, Box::new(e)))?;

    // s3s::Body supports From<Vec<u8>>
    let body = s3s::Body::from(data.clone());

    // Basic mime detection
    let content_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();

    let mut output = GetObjectOutput::default();
    output.body = Some(body.into());
    output.content_length = Some(data.len() as i64);
    output.content_type = Some(content_type);

    Ok(S3Response::new(output))
}

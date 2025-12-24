use super::super::LocalS3;
use crate::config::LocalConfig;
use axum::http::StatusCode;
use s3s::service::S3ServiceBuilder;

pub fn create_service(
    config: &LocalConfig,
    s3_impl: LocalS3,
) -> impl tower::Service<
    axum::http::Request<axum::body::Body>,
    Response = axum::http::Response<axum::body::Body>,
    Error = std::convert::Infallible,
    Future = impl std::future::Future<
        Output = Result<axum::http::Response<axum::body::Body>, std::convert::Infallible>,
    > + Send,
> + Clone {
    // Auth
    let access_key = config.aws_access_key_id.clone();
    let secret_key = config.aws_secret_access_key.clone();
    let simple_auth = s3s::auth::SimpleAuth::from_single(access_key, secret_key);

    // Create two services: one public (for GET), one authenticated (for PUT/DELETE)
    let public_service = S3ServiceBuilder::new(s3_impl.clone()).build();
    let mut auth_builder = S3ServiceBuilder::new(s3_impl);
    auth_builder.set_auth(simple_auth);
    let auth_service = auth_builder.build();

    // Combined service that dispatches based on method
    tower::service_fn(move |req: axum::http::Request<axum::body::Body>| {
        let public_service = public_service.clone();
        let auth_service = auth_service.clone();

        async move {
            let (parts, body) = req.into_parts();
            let method = parts.method.clone();

            // Read the body eagerly to convert to s3s::Body (required for both services)
            let bytes = match axum::body::to_bytes(body, usize::MAX).await {
                Ok(b) => b,
                Err(e) => {
                    return Ok::<_, std::convert::Infallible>(
                        axum::http::Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(axum::body::Body::from(format!(
                                "Failed to read body: {}",
                                e
                            )))
                            .unwrap(),
                    )
                }
            };

            // Reconstruct request for s3s
            let s3_req = axum::http::Request::from_parts(parts, s3s::Body::from(bytes));

            // Extract path to determine bucket
            let path = s3_req.uri().path();

            // Dispatch based on Bucket Policy
            let mut is_public_read = false;

            // Extract bucket name from path
            // Path structure: /<bucket>/<key>
            if let Some(path_str) = path.strip_prefix('/') {
                if let Some((bucket_name, _)) = path_str.split_once('/') {
                    if let Some(bucket) = super::bucket::find(bucket_name) {
                        if bucket.access == super::bucket::BucketAccess::PublicRead
                            && method == axum::http::Method::GET
                        {
                            is_public_read = true;
                        }
                    }
                }
            }

            let s3_resp = if is_public_read {
                public_service.call(s3_req).await
            } else {
                auth_service.call(s3_req).await
            };

            match s3_resp {
                Ok(res) => {
                    let (parts, body) = res.into_parts();
                    let body = axum::body::Body::from_stream(body);
                    Ok(axum::http::Response::from_parts(parts, body))
                }
                Err(err) => Ok(axum::http::Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(axum::body::Body::from(format!("S3 Error: {:?}", err)))
                    .unwrap()),
            }
        }
    })
}

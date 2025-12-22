mod av;
mod hls;
mod queue;

use crate::queue::{enqueue_video, RedisQueue, WorkerPool};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, State},
    http::StatusCode,
    routing::post,
    BoxError, Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use dotenv::dotenv;
use futures::{Stream, TryStreamExt};
use std::env;
use std::io;
use std::path::PathBuf;
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;

use serde::Deserialize;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let addr: String = env::var("ADDR").unwrap_or_else(|_| String::from("127.0.0.1"));
    let port: String = env::var("PORT").unwrap_or_else(|_| String::from("3000"));

    tracing_subscriber::fmt::init();

    // Initialize Redis queue
    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| String::from("redis://127.0.0.1/"));
    let queue = match RedisQueue::new(&redis_url) {
        Ok(q) => Arc::new(q),
        Err(e) => {
            eprintln!("Failed to connect to Redis: {:?}", e);
            eprintln!("Make sure Redis is running at {}", redis_url);
            std::process::exit(1);
        }
    };

    // Start worker pool
    let pool = WorkerPool::new(queue.clone());
    let _workers = pool.start();
    println!(
        "Started {} transcoding workers",
        crate::queue::WORKERS_COUNT
    );

    // Get HLS directory for serving transcoded files
    let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| String::from("~/"));
    let hls_dir = PathBuf::from(&upload_dir).join("hls");

    // CORS layer for the viewer app - explicit configuration for better Firefox compatibility
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        .route("/upload", post(upload_media))
        .nest_service("/hls", ServeDir::new(&hls_dir))
        .layer(cors)
        .layer(DefaultBodyLimit::disable())
        .with_state(queue);

    println!("Serving HLS files from {:?}", hls_dir);

    let app = router;
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", addr, port))
        .await
        .expect("Failed to bind TCP listener");
    println!("Listening at {}:{}", addr, port);
    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}

// Handler that accepts a multipart form upload and streams each field to a file.
async fn upload_media(
    State(queue): State<Arc<RedisQueue>>,
    mut multipart: Multipart,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let mut uploaded_files = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = if let Some(file_name) = field.file_name() {
            file_name.to_owned()
        } else {
            continue;
        };

        let path =
            std::path::Path::new(&env::var("WAITING_DIR").unwrap_or_else(|_| String::from("~/")))
                .to_path_buf();
        if !path_is_valid(&path) {
            return Err((StatusCode::BAD_REQUEST, "Invalid path".to_owned()));
        }

        let path = path.join(&file_name);
        println!("Saving new file to {:?}", path);
        stream_to_file(&path, field).await?;

        // Enqueue video for processing
        if let Err(e) = enqueue_video(&queue, &path).await {
            eprintln!("Failed to enqueue video: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to queue video: {:?}", e),
            ));
        }

        uploaded_files.push(file_name);
    }

    Ok(axum::Json(serde_json::json!({
        "status": "ok",
        "message": "Upload successful",
        "files": uploaded_files
    })))
}

#[derive(Debug, Deserialize)]
struct MediaStreamRequest {
    pub uri: String,
    pub audio_track_id: u8,
    pub no_video: bool,
}

// Save a `Stream` to a file
async fn stream_to_file<S, E>(path: &PathBuf, stream: S) -> Result<(), (StatusCode, String)>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    async {
        let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
        let body_reader = StreamReader::new(body_with_io_error);
        futures::pin_mut!(body_reader);

        let mut file = BufWriter::new(File::create(path).await?);
        tokio::io::copy(&mut body_reader, &mut file).await?;

        Ok::<_, io::Error>(())
    }
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

fn path_is_valid(path: &PathBuf) -> bool {
    println!("Checking path {:?}", path);
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            println!("Invalid Path: Contains ParentDir");
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::stream;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use tokio;

    #[tokio::test]
    async fn test_stream_to_file() {
        // Create a temporary directory for the test
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");

        type E = std::io::Error;

        // Create a mock stream
        let test_data = "Hello, world!";
        let mock_stream = stream::iter(vec![Ok::<bytes::Bytes, E>(Bytes::from(test_data))]);

        // Call the function
        let result = stream_to_file(&file_path, mock_stream).await;

        // Check the result
        assert!(result.is_ok());

        // Verify the file contents
        let file_contents = fs::read_to_string(file_path).unwrap();
        assert_eq!(file_contents, test_data);
    }

    #[tokio::test]
    async fn test_stream_to_file_error() {
        // Create a temporary directory for the test
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");

        // Create a mock stream that returns an error
        let mock_stream = stream::iter(vec![Err("Test error")]);

        // Call the function
        let result = stream_to_file(&file_path, mock_stream).await;

        // Check the result
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            (StatusCode::INTERNAL_SERVER_ERROR, "Test error".to_string())
        );
    }

    #[test]
    fn test_valid_path() {
        let valid_path = PathBuf::from("valid_directory");
        assert!(path_is_valid(&valid_path));
    }

    #[test]
    fn test_path_with_parent() {
        let invalid_path = PathBuf::from("../invalid_directory");
        assert!(!path_is_valid(&invalid_path));
    }

    #[test]
    fn test_path_with_multiple_components() {
        let path = PathBuf::from("dir1/dir2");
        assert!(path_is_valid(&path));
    }

    #[test]
    fn test_path_with_root() {
        let path = PathBuf::from("/root_directory");
        assert!(path_is_valid(&path));
    }
}

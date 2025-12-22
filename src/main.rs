mod av;
mod queue;

use crate::queue::MAX_CONCURRENT_VIDEOS;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, State},
    http::StatusCode,
    response::{Html, Redirect},
    routing::{get, post},
    BoxError, Router,
};
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::queue::add_to_queue;
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
    let is_test: bool = env::var("IS_TEST")
        .unwrap_or_else(|_| String::from("true"))
        .parse()
        .unwrap_or(true);

    tracing_subscriber::fmt::init();

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_VIDEOS));

    let mut router = Router::new()
        .route("/upload", post(upload_media))
        .layer(DefaultBodyLimit::disable())
        .with_state(semaphore);

    if is_test {
        router = router.route("/", get(root));
    }

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
    State(semaphore): State<Arc<Semaphore>>,
    mut multipart: Multipart,
) -> Result<Redirect, (StatusCode, String)> {
    let upload_dir = match env::var("UPLOAD_DIR") {
        Ok(dir) => dir,
        Err(_) => String::from("~/"),
    };

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
        add_to_queue(semaphore.clone(), &path).await;
    }

    Ok(Redirect::to("/"))
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
    fn test_invalid_path_with_parent() {
        let invalid_path = PathBuf::from("../invalid_directory");
        assert!(!path_is_valid(&invalid_path));
    }

    #[test]
    fn test_invalid_path_with_multiple_components() {
        let invalid_path = PathBuf::from("dir1/dir2");
        assert!(!path_is_valid(&invalid_path));
    }

    #[test]
    fn test_invalid_path_with_root() {
        let invalid_path = PathBuf::from("/root_directory");
        assert!(!path_is_valid(&invalid_path));
    }
}

async fn root() -> Html<String> {
    dotenv().ok();
    let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| String::from("~/"));
    println!("{}", upload_dir);
    let files = match std::fs::read_dir(upload_dir) {
        Ok(entries) => entries
            .filter_map(|entry| {
                entry
                    .ok()
                    .and_then(|e| e.file_name().to_str().map(String::from))
            })
            .collect::<Vec<String>>(),
        Err(_) => vec!["Error reading directory".to_string()],
    };

    let file_list = files
        .iter()
        .map(|file| format!("<li>{}</li>", file))
        .collect::<String>();

    Html(format!(
        r#"
        <!doctype html>
        <html>
            <head>
                <title>Upload something!</title>
            </head>
            <body>
                <h1>Files in upload directory:</h1>
                <ul>{}</ul>
                <form action="/upload" method="post" enctype="multipart/form-data">
                    <div>
                        <label>
                            Upload file:
                            <input type="file" name="file" multiple>
                        </label>
                    </div>
                    <div>
                        <input type="submit" value="Upload files">
                    </div>
                </form>
            </body>
        </html>
        "#,
        file_list
    ))
}

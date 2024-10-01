
mod queue;
mod av;

use axum::{body::Bytes, routing::{get, post}, response::{Html, Redirect}, extract::{Multipart, DefaultBodyLimit}, http::StatusCode, Router, BoxError};

use std::io;
use std::path::PathBuf;
use futures::{Stream, TryStreamExt};
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;
use crate::queue::process_video;
use dotenv::dotenv;
use std::env;

use serde::Deserialize;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let addr: String = env::var("ADDR").unwrap_or_else(|_| String::from("127.0.0.1"));
    let port: String = env::var("PORT").unwrap_or_else(|_| String::from("3000"));
    let is_test: bool = env::var("IS_TEST").unwrap_or_else(|_| String::from("true")).parse().unwrap_or(true);

    tracing_subscriber::fmt::init();

    let mut router = Router::new()
        .route("/upload", post(upload_media))
        .layer(DefaultBodyLimit::disable());

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
async fn upload_media(mut multipart: Multipart) -> Result<Redirect, (StatusCode, String)> {
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

        let path = std::path::Path::new(&env::var("WAITING_DIR").unwrap_or_else(|_| String::from("~/"))).to_path_buf();
        if !path_is_valid(&path) {
            return Err((StatusCode::BAD_REQUEST, "Invalid path".to_owned()));
        }

        let path = path.join(&file_name);
        println!("Saving new file to {:?}", path);
        stream_to_file(&path, field).await?;
        process_video(&path).await;
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
    let mut components = path.components().peekable();
    if let Some(first) = components.peek() {
        if !matches!(first, std::path::Component::Normal(_)) {
            println!("Unvalid Path");
            return false;
        }
    }

    components.count() == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use bytes::Bytes;
    use futures::stream;
    use std::fs;
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

// basic handler that responds with a static string
async fn root() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head>
                <title>Upload something!</title>
            </head>
            <body>
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
    )
}
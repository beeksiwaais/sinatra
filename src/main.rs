
mod queue;
mod video;

use axum::{body::Bytes, routing::{get, post}, response::{Html, Redirect}, extract::{Multipart, DefaultBodyLimit}, http::StatusCode, Router, BoxError};

use std::io;
use std::path::PathBuf;
use futures::{Stream, TryStreamExt};
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;
use crate::queue::process_video;

use serde::Deserialize;

const ADDR: &str = "127.0.0.1";
const PORT: &str = "3000";
const UPLOAD_DR: &str = "~/";
const WAITING_DIR: &str = "~/";
const IS_TEST: bool = true;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let router = Router::new()
        .route("/upload", post(upload_media))
        .layer(DefaultBodyLimit::disable())
        .route("/", get(root));

    // build our application with a route
    let app = router;
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", ADDR, PORT)).await.unwrap();
    println!("Listening at {}:{}", ADDR, PORT);
    axum::serve(listener, app).await.unwrap();
}

async fn get_media() {}

// Handler that accepts a multipart form upload and streams each field to a file.
async fn upload_media(mut multipart: Multipart) -> Result<Redirect, (StatusCode, String)> {
    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = if let Some(file_name) = field.file_name() {
            file_name.to_owned()
        } else {
            continue;
        };

        let path = std::path::Path::new(WAITING_DIR).to_path_buf();
        //if !path_is_valid(&path) {
        //    return Err((StatusCode::BAD_REQUEST, "Invalid path".to_owned()));
        //}

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
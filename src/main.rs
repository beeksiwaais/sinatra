
mod queue;
mod video;

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

    // initialize tracing
    tracing_subscriber::fmt::init();

    let router = Router::new()
        .route("/upload", post(upload_media))
        .layer(DefaultBodyLimit::disable())
        .route("/", get(root));

    // build our application with a route
    let app = router;
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", addr, port)).await.unwrap();
    println!("Listening at {}:{}", addr, port);
    axum::serve(listener, app).await.unwrap();
}

// Handler that accepts a multipart form upload and streams each field to a file.
async fn upload_media(mut multipart: Multipart) -> Result<Redirect, (StatusCode, String)> {
    let upload_dr: &str = env::var("UPLOAD_DR").unwrap_or_else(|_| String::from("~/"));
    let waiting_dir: &str = env::var("WAITING_DIR").unwrap_or_else(|_| String::from("~/"));


    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = if let Some(file_name) = field.file_name() {
            file_name.to_owned()
        } else {
            continue;
        };

        let path = std::path::Path::new(waiting_dir).to_path_buf();
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
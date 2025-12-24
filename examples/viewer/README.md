# Sinatra HLS Viewer

A Rust-based web viewer for the Sinatra HLS transcoding pipeline.

## Quick Start

```bash
# Terminal 1: Start the main server
UPLOAD_DIR=./test_vars WAITING_DIR=./test_vars/waiting cargo run

# Terminal 2: Start the viewer
cargo run --example viewer
```

Then open http://127.0.0.1:8080

## Features

- **Drag & Drop**: Drop video files to upload and transcode
- **HLS Streaming**: Watch via HLS.js as segments complete  
- **Progress Tracking**: Visual feedback during transcoding
- **Modern UI**: Glassmorphism design with smooth animations

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `VIEWER_PORT` | 8080 | Port for the viewer |

## Architecture

```
┌──────────────────┐     ┌──────────────────┐
│  Viewer (:8080)  │     │  Server (:3000)  │
│  GET /presign    │     │  PUT /stream/*   │
└────────┬─────────┘     │  GET /hls/*      │
         │               └────────▲─────────┘
         │                        │
         │   (Presigned URL)      │
         └───────────▶ Upload ────┘
```

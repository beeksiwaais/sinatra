use super::av::AV;
use ffmpeg_next as ffmpeg;
use std::path::PathBuf;
use tokio::fs;

use tokio::process::Command;
use tokio::task;

pub async fn get_segments(path: &std::path::Path) -> Vec<f64> {
    let path_clone = path.to_path_buf();

    task::spawn_blocking(move || {
        ffmpeg::init().unwrap();
        match ffmpeg::format::input(&path_clone) {
            Ok(mut context) => {
                let stream_index = context
                    .streams()
                    .best(ffmpeg::media::Type::Video)
                    .map(|stream| stream.index());

                if let Some(stream_index) = stream_index {
                    let time_base = context.stream(stream_index).unwrap().time_base();
                    let time_base_f64 =
                        time_base.numerator() as f64 / time_base.denominator() as f64;

                    let mut segments = Vec::new();

                    for (stream, packet) in context.packets() {
                        if stream.index() == stream_index && packet.is_key() {
                            if let Some(pts) = packet.pts() {
                                let time = pts as f64 * time_base_f64;
                                segments.push(time);
                            }
                        }
                    }
                    segments
                } else {
                    eprintln!("No video stream found");
                    Vec::new()
                }
            }
            Err(e) => {
                eprintln!("Error opening input: {}", e);
                Vec::new()
            }
        }
    })
    .await
    .unwrap()
}

pub async fn transcode_at(av: &AV<'_>, segment: usize, at_path: PathBuf) {
    if segment >= av.segments.len() {
        println!(
            "Segment {:?} was not transcoded because it do not match known segments in av",
            segment
        );
        return;
    }

    let start_at = av.segments.get(segment).unwrap().to_string();
    let duration: f64 = av.segments.get(segment + 1).unwrap() - av.segments.get(segment).unwrap();
    let duration_as_str: String = duration.to_string();

    // Use a temporary path for the full fMP4 (header + fragment)
    let temp_path = at_path.with_extension("temp.mp4");

    let transcode = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(&start_at)
        .arg("-i")
        .arg(av.path)
        .arg("-t")
        .arg(&duration_as_str)
        // Fragmented MP4 flags for HLS compatibility
        .arg("-movflags")
        .arg("frag_keyframe+empty_moov+default_base_moof")
        .arg("-output_ts_offset")
        .arg(&start_at)
        .arg(&temp_path)
        .output()
        .await;

    println!("Transcode result for segment {}: {:?}", segment, transcode);

    if let Ok(output) = transcode {
        if !output.status.success() {
            eprintln!("FFmpeg failed: {:?}", output);
            return;
        }

        // Strip the initialization header (ftyp + moov) to leave only the fragment (moof + mdat)
        if let Err(e) = strip_init_header(&temp_path, &at_path).await {
            eprintln!("Failed to strip header for segment {}: {}", segment, e);
        } else {
            // Clean up temp file only on success
            let _ = fs::remove_file(temp_path).await;
        }
    }
}

/// Helper to strip ftyp+moov from a fragmented MP4, leaving only the fragment.
async fn strip_init_header(
    input_path: &PathBuf,
    output_path: &PathBuf,
) -> Result<(), std::io::Error> {
    let data = fs::read(input_path).await?;

    let mut offset = 0;
    while offset < data.len() {
        if offset + 8 > data.len() {
            break;
        }

        let size = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        if size < 8 || offset + size > data.len() {
            break;
        }

        let box_type = std::str::from_utf8(&data[offset + 4..offset + 8]).unwrap_or("");

        if box_type == "moof" {
            // Found the fragment start. Write everything from here to end.
            fs::write(output_path, &data[offset..]).await?;
            return Ok(());
        }
        offset += size;
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "No moof atom found",
    ))
}

/// Generate a standalone init.mp4 from the source file.
/// This runs a quick transcoding of 1 frame to generate the headers.
#[allow(dead_code)]
pub async fn generate_init_segment(
    source_path: &std::path::Path,
    init_path: &std::path::Path,
) -> Result<(), std::io::Error> {
    // We generate a temp fMP4 to extract the header from
    let temp_init_path = init_path.with_extension("init_temp.mp4");

    let _ = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(source_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-movflags")
        .arg("frag_keyframe+empty_moov+default_base_moof")
        .arg(&temp_init_path)
        .output()
        .await?;

    // Now parse and extract only ftyp + moov
    let data = fs::read(&temp_init_path).await?;
    let mut init_data = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        if offset + 8 > data.len() {
            break;
        }
        let size = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        if size < 8 || offset + size > data.len() {
            break;
        }
        let box_type = std::str::from_utf8(&data[offset + 4..offset + 8]).unwrap_or("");

        match box_type {
            "ftyp" | "moov" => {
                init_data.extend_from_slice(&data[offset..offset + size]);
            }
            "moof" => {
                break; // Stop at first fragment
            }
            _ => {}
        }
        offset += size;
    }

    if !init_data.is_empty() {
        fs::write(init_path, &init_data).await?;
        println!("Generated init segment at {:?}", init_path);
    }

    // Clean up
    let _ = fs::remove_file(temp_init_path).await;

    Ok(())
}

#[cfg(test)]
#[tokio::test]
async fn test_parallel_transcoding() {
    use super::av::AV;

    // Setup paths
    let source_str = "test_vars/hls/ssstik.io_@souk.henna_1766442357114/segment_1.mp4";
    let source = PathBuf::from(source_str);

    // If running in a context where test_vars isn't relative to CWD, try to find it
    if !source.exists() {
        println!("Skipping test: {:?} not found", source);
        return;
    }

    let temp_dir = std::env::temp_dir();
    let init_out = temp_dir.join("init_verif.mp4");
    let seg_out = temp_dir.join("seg_0_verif.m4s");

    // 1. Test Init Generation
    // This confirms we can pull the header from the source
    let init_res = generate_init_segment(&source, &init_out).await;
    assert!(init_res.is_ok(), "generate_init_segment failed");

    let init_data = fs::read(&init_out).await.unwrap();
    // Check for ftyp tag at offset 4
    assert_eq!(
        &init_data[4..8],
        b"ftyp",
        "Init segment should start with ftyp"
    );

    // 2. Test Segment Transcoding
    // We construct a mock AV.
    // Note: The `path` in AV uses the same lifetime as AV.
    let av = AV {
        path: &source,
        video_streams: vec![],
        audio_streams: vec![],
        segments: vec![0.0, 0.5], // Trancode first 0.5s
    };

    transcode_at(&av, 0, seg_out.clone()).await;

    // 3. Verify Segment Content
    let seg_data = fs::read(&seg_out).await.unwrap();
    // The strip function looks for 'moof' and writes from there.
    // So the output file should start directly with the moof box.
    // Box structure: [size: 4 bytes] [type: 4 bytes] ...
    if seg_data.len() > 8 {
        let box_type = std::str::from_utf8(&seg_data[4..8]).unwrap_or("");
        assert_eq!(
            box_type, "moof",
            "Segment should start with moof atom (header stripped)"
        );
    } else {
        panic!("Generated segment is too short");
    }

    // Cleanup
    let _ = fs::remove_file(init_out).await;
    let _ = fs::remove_file(seg_out).await;
}

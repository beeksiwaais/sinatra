use crate::av::av::AV;
use regex::Regex;
use std::path::PathBuf;
use tokio::process::Command;

pub async fn get_segments(path: &PathBuf) -> Vec<f64> {
    let output = Command::new("ffprobe")
        .arg("-select_streams")
        .arg("v")
        .arg("-skip_frame")
        .arg("nokey")
        .arg("-show_frames")
        .arg("-v")
        .arg("quiet")
        .arg(path)
        .output()
        .await;

    let segment = output.unwrap().stdout;
    let re = Regex::new(r"pts_time=(\d+\.\d+)").unwrap();
    let stdout_str = &*String::from_utf8_lossy(&segment);

    return stdout_str
        .lines()
        .filter_map(|line| {
            if re.is_match(line) {
                let caps = re.captures(line).unwrap();
                println!("{:?}", caps);
                Some(caps.get(1).unwrap().as_str())
            } else {
                None
            }
        })
        // Print the processed lines
        .map(|processed_line| {
            println!("{}", processed_line);
            processed_line.parse::<f64>().unwrap()
        })
        .collect();
}

pub async fn transcode_at(av: &AV<'_>, segment: usize, at_path: PathBuf) {
    if segment >= av.segments.len() {
        println!(
            "Segment {:?} was not transcoded because it do not match known segments in av",
            segment
        );
    }

    let start_at = av.segments.get(segment).unwrap().to_string();
    let duration: f64 = av.segments.get(segment + 1).unwrap() - av.segments.get(segment).unwrap();
    let duration_as_str: String = duration.to_string();

    let transcode = Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg(&start_at)
        //.arg("-itsoffset")
        //.arg(&start_at)
        .arg("-i")
        .arg(av.path)
        .arg("-t")
        .arg(&duration_as_str)
        // Fragmented MP4 flags for HLS compatibility
        .arg("-movflags")
        .arg("frag_keyframe+empty_moov+default_base_moof")
        //.arg("-codec")
        //.arg("copy")
        .arg(at_path.clone())
        .output()
        .await;

    println!("{:?}", transcode);
}

/// Extract the initialization segment (ftyp+moov) from the first media segment.
/// For fMP4 HLS, we need a separate init.mp4 containing just the moov box.
pub async fn generate_init_segment(
    first_segment: &PathBuf,
    init_path: &PathBuf,
) -> Result<(), std::io::Error> {
    use tokio::fs;
    use tokio::io::AsyncReadExt;

    let mut file = fs::File::open(first_segment).await?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).await?;

    // Parse MP4 boxes to find ftyp and moov
    let mut offset = 0;
    let mut init_data = Vec::new();

    while offset < data.len() {
        if offset + 8 > data.len() {
            break;
        }

        // Read box size (big endian)
        let size = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;

        if size < 8 || offset + size > data.len() {
            break;
        }

        // Read box type
        let box_type = std::str::from_utf8(&data[offset + 4..offset + 8]).unwrap_or("");

        match box_type {
            "ftyp" | "moov" => {
                init_data.extend_from_slice(&data[offset..offset + size]);
            }
            "moof" => {
                // Stop when we hit media fragments
                break;
            }
            _ => {}
        }

        offset += size;
    }

    if !init_data.is_empty() {
        fs::write(init_path, &init_data).await?;
        println!("Generated init segment at {:?}", init_path);
    }

    Ok(())
}

use super::av::AV;
use ffmpeg::{codec, encoder, format, media, Dictionary, Rational};
use ffmpeg_next as ffmpeg;
use std::path::{Path, PathBuf};
use tokio::fs;

use tokio::task;

/// Muxer flags shared by the init segment and every media segment, so the fragments
/// we emit stay compatible with the header a player has already loaded.
const FRAGMENTED_MP4_FLAGS: &str = "frag_keyframe+empty_moov+default_base_moof";

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

/// Copy the streams of `source` into a fragmented MP4 at `dest`, without re-encoding.
///
/// `range` is an optional `(start, duration)` in seconds selecting which packets to
/// copy; `None` writes the header and nothing else. Note that each call is its own
/// muxer, so the fragment's baseMediaDecodeTime restarts at 0 rather than carrying
/// the absolute presentation time.
///
/// Returns the byte length of the initialization section (ftyp + moov), which is
/// exactly where the first fragment begins.
fn remux_fragmented(
    source: &Path,
    dest: &Path,
    range: Option<(f64, f64)>,
) -> Result<u64, ffmpeg::Error> {
    ffmpeg::init()?;

    let mut ictx = format::input(&source)?;
    let mut octx = format::output_as(&dest, "mp4")?;

    // Map audio/video/subtitle streams across, copying codec parameters verbatim.
    let mut stream_mapping = vec![-1i32; ictx.nb_streams() as usize];
    let mut ist_time_bases = vec![Rational(0, 1); ictx.nb_streams() as usize];
    let mut mapped = Vec::new();
    let mut ost_index = 0;

    for (ist_index, ist) in ictx.streams().enumerate() {
        let medium = ist.parameters().medium();
        if medium != media::Type::Audio
            && medium != media::Type::Video
            && medium != media::Type::Subtitle
        {
            continue;
        }

        stream_mapping[ist_index] = ost_index;
        ist_time_bases[ist_index] = ist.time_base();
        mapped.push(ist_index);
        ost_index += 1;

        let mut ost = octx.add_stream(encoder::find(codec::Id::None))?;
        ost.set_parameters(ist.parameters());
        // Codec tags are container specific and don't carry over between muxers.
        unsafe {
            (*ost.parameters().as_mut_ptr()).codec_tag = 0;
        }
    }

    let mut options = Dictionary::new();
    options.set("movflags", FRAGMENTED_MP4_FLAGS);
    octx.write_header_with(options)?;

    // `empty_moov` means the header is complete as soon as write_header returns, so
    // the current output position is the exact size of the init segment. avio_tell is
    // a static inline in C and therefore not bound, so seek by 0 from SEEK_CUR (1).
    let init_size = unsafe {
        let position = ffmpeg::ffi::avio_seek((*octx.as_mut_ptr()).pb, 0, 1);
        if position < 0 {
            return Err(ffmpeg::Error::from(position as i32));
        }
        position as u64
    };

    let Some((start, duration)) = range else {
        return Ok(init_size);
    };
    let end = start + duration;

    // Seek to the keyframe at or before `start`; segment boundaries come from
    // get_segments, so this normally lands exactly on the requested keyframe.
    let seek_target = (start * f64::from(ffmpeg::ffi::AV_TIME_BASE)) as i64;
    ictx.seek(seek_target, ..seek_target)?;

    let mut past_end = vec![false; stream_mapping.len()];

    for (stream, mut packet) in ictx.packets() {
        let ist_index = stream.index();
        let ost_index = stream_mapping[ist_index];
        if ost_index < 0 {
            continue;
        }

        let ist_time_base = ist_time_bases[ist_index];
        if let Some(pts) = packet.pts() {
            let time =
                pts as f64 * ist_time_base.numerator() as f64 / ist_time_base.denominator() as f64;

            if time < start {
                continue;
            }
            if time >= end {
                // Other streams may still owe us packets for this window.
                past_end[ist_index] = true;
                if mapped.iter().all(|&index| past_end[index]) {
                    break;
                }
                continue;
            }
        }

        let ost_time_base = octx.stream(ost_index as usize).unwrap().time_base();
        packet.rescale_ts(ist_time_base, ost_time_base);
        packet.set_position(-1);
        packet.set_stream(ost_index as usize);
        packet.write_interleaved(&mut octx)?;
    }

    // For fragmented MP4 av_write_trailer returns the size of the trailing mfra box,
    // and ffmpeg-next's write_trailer() reports any non-zero return as an error, so
    // call it directly and only treat a negative result as a failure.
    let trailer = unsafe { ffmpeg::ffi::av_write_trailer(octx.as_mut_ptr()) };
    if trailer < 0 {
        return Err(ffmpeg::Error::from(trailer));
    }

    Ok(init_size)
}

pub async fn transcode_at(av: &AV<'_>, segment: usize, at_path: PathBuf) {
    if segment + 1 >= av.segments.len() {
        println!(
            "Segment {:?} was not transcoded because it do not match known segments in av",
            segment
        );
        return;
    }

    let start_at = av.segments[segment];
    let duration = av.segments[segment + 1] - start_at;

    // Use a temporary path for the full fMP4 (header + fragment)
    let temp_path = at_path.with_extension("temp.mp4");

    let source = av.path.to_path_buf();
    let remux_target = temp_path.clone();
    let remuxed = task::spawn_blocking(move || {
        remux_fragmented(&source, &remux_target, Some((start_at, duration)))
    })
    .await
    .unwrap();

    let init_size = match remuxed {
        Ok(init_size) => init_size as usize,
        Err(e) => {
            eprintln!("FFmpeg failed for segment {}: {}", segment, e);
            let _ = fs::remove_file(temp_path).await;
            return;
        }
    };

    // Drop the initialization header (ftyp + moov) to leave only the fragment
    // (moof + mdat); players load that header once, from the init segment.
    match fs::read(&temp_path).await {
        Ok(data) if data.len() > init_size => {
            if let Err(e) = fs::write(&at_path, &data[init_size..]).await {
                eprintln!("Failed to write segment {}: {}", segment, e);
                return;
            }
            let _ = fs::remove_file(temp_path).await;
        }
        Ok(_) => eprintln!("Segment {} contains no fragment data", segment),
        Err(e) => eprintln!("Failed to read segment {}: {}", segment, e),
    }
}

/// Generate a standalone init.mp4 from the source file.
/// Only the muxer header is written, so the result is exactly ftyp + moov.
#[allow(dead_code)]
pub async fn generate_init_segment(
    source_path: &std::path::Path,
    init_path: &std::path::Path,
) -> Result<(), std::io::Error> {
    let source = source_path.to_path_buf();
    let destination = init_path.to_path_buf();

    let init_size = task::spawn_blocking(move || remux_fragmented(&source, &destination, None))
        .await
        .unwrap()
        .map_err(std::io::Error::other)?;

    // Nothing follows the header, but truncate anyway so the file is exactly the
    // init segment regardless of what the muxer decided to flush.
    let file = fs::OpenOptions::new().write(true).open(init_path).await?;
    file.set_len(init_size).await?;

    println!("Generated init segment at {:?}", init_path);

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

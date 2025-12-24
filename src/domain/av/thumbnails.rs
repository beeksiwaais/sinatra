use ffmpeg_next as ffmpeg;
use std::path::Path;

pub async fn generate_strip(
    source: &Path,
    output: &Path,
    interval_seconds: u32,
    width: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let source = source.to_path_buf();
    let output = output.to_path_buf();

    tokio::task::spawn_blocking(
        move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            ffmpeg::init()?;

            // Input
            let mut ictx = ffmpeg::format::input(&source)?;
            let input_stream = ictx
                .streams()
                .best(ffmpeg::media::Type::Video)
                .ok_or(ffmpeg::Error::StreamNotFound)?;
            let stream_index = input_stream.index();

            // Decoder context
            let context_decoder =
                ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
            let mut decoder = context_decoder.decoder().video()?;

            // Filter Graph
            let mut graph = ffmpeg::filter::Graph::new();
            let args = format!(
                "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
                decoder.width(),
                decoder.height(),
                decoder.format() as i32,
                input_stream.time_base().numerator(),
                input_stream.time_base().denominator(),
                decoder.aspect_ratio().numerator(),
                decoder.aspect_ratio().denominator()
            );

            graph.add(&ffmpeg::filter::find("buffer").unwrap(), "in", &args)?;
            graph.add(&ffmpeg::filter::find("buffersink").unwrap(), "out", "")?;

            // fps=1/interval, scale=width:-1, tile=layout needed?
            // Use [in] and [out] labels to connect to our buffer and buffersink
            let filter_spec = format!(
                "[in]fps=1/{},scale={}:-1,tile=5x5[out]",
                interval_seconds, width
            );

            graph.parse(&filter_spec)?;
            graph.validate()?;

            let mut source_filter = graph.get("in").unwrap();
            let mut sink_filter = graph.get("out").unwrap();

            // Process
            let mut decoded_frame = ffmpeg::util::frame::Video::empty();
            let mut filtered_frame = ffmpeg::util::frame::Video::empty();

            for (stream, packet) in ictx.packets() {
                if stream.index() == stream_index {
                    decoder.send_packet(&packet)?;
                    while decoder.receive_frame(&mut decoded_frame).is_ok() {
                        // Send frame to graph
                        source_filter.source().add(&decoded_frame)?;
                    }
                }
            }
            // Flush decoder
            decoder.send_eof()?;
            while decoder.receive_frame(&mut decoded_frame).is_ok() {
                source_filter.source().add(&decoded_frame)?;
            }

            // Flush graph
            source_filter.source().flush()?;

            // Get output frames (likely just one for a tile, or multiple if long video)
            // We will just save the first one for now as "thumbnails.jpg".
            // Realistically, tiling logic outputs a frame whenever the grid is full.
            // If we want a single strip, 'tile' logic needs to know total frames or we handle it differently.
            // For now, let's just save whatever comes out.

            if sink_filter.sink().frame(&mut filtered_frame).is_ok() {
                // Convert to RGB for saving with image crate
                let mut rgb_frame = ffmpeg::util::frame::Video::empty();
                let mut scaler = ffmpeg::software::scaling::context::Context::get(
                    filtered_frame.format(),
                    filtered_frame.width(),
                    filtered_frame.height(),
                    ffmpeg::format::Pixel::RGB24,
                    filtered_frame.width(),
                    filtered_frame.height(),
                    ffmpeg::software::scaling::flag::Flags::BILINEAR,
                )?;
                scaler.run(&filtered_frame, &mut rgb_frame)?;

                let img_buffer = image::RgbImage::from_raw(
                    rgb_frame.width(),
                    rgb_frame.height(),
                    rgb_frame.data(0).to_vec(),
                )
                .ok_or("Failed to create image buffer")?;

                img_buffer.save(&output)?;
                println!("Saved thumbnail strip to {:?}", output);
            }

            Ok(())
        },
    )
    .await?
}

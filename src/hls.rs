use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub struct MediaSegment {
    pub duration: f64,
    pub uri: String,
}

pub struct MediaPlaylist {
    pub version: u8,
    pub target_duration: u64,
    pub media_sequence: u64,
    pub segments: Vec<MediaSegment>,
    pub end_list: bool,
}

impl MediaPlaylist {
    pub fn new(target_duration: u64) -> Self {
        Self {
            version: 3,
            target_duration,
            media_sequence: 0,
            segments: Vec::new(),
            end_list: true,
        }
    }

    pub fn add_segment(&mut self, duration: f64, uri: String) {
        self.segments.push(MediaSegment { duration, uri });
    }

    pub async fn write_to(&self, path: &PathBuf) -> Result<(), std::io::Error> {
        let mut file = File::create(path).await?;

        file.write_all(b"#EXTM3U\n").await?;
        file.write_all(format!("#EXT-X-VERSION:{}\n", self.version).as_bytes())
            .await?;
        file.write_all(format!("#EXT-X-TARGETDURATION:{}\n", self.target_duration).as_bytes())
            .await?;
        file.write_all(format!("#EXT-X-MEDIA-SEQUENCE:{}\n", self.media_sequence).as_bytes())
            .await?;

        for segment in &self.segments {
            // Using {:.6} for reasonable precision on float duration
            file.write_all(format!("#EXTINF:{:.6},\n", segment.duration).as_bytes())
                .await?;
            file.write_all(segment.uri.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }

        if self.end_list {
            file.write_all(b"#EXT-X-ENDLIST\n").await?;
        }

        Ok(())
    }
}

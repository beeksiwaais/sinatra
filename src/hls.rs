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
    pub playlist_type: Option<String>,
    pub independent_segments: bool,
    /// Initialization segment for fMP4 (EXT-X-MAP)
    pub init_segment: Option<String>,
}

impl MediaPlaylist {
    pub fn new(target_duration: u64) -> Self {
        Self {
            version: 7, // Version 7 for fMP4 support
            target_duration,
            media_sequence: 0,
            segments: Vec::new(),
            end_list: true,
            playlist_type: None,
            independent_segments: false,
            init_segment: None,
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

        if let Some(pt) = &self.playlist_type {
            file.write_all(format!("#EXT-X-PLAYLIST-TYPE:{}\n", pt).as_bytes())
                .await?;
        }

        if self.independent_segments {
            file.write_all(b"#EXT-X-INDEPENDENT-SEGMENTS\n").await?;
        }

        // fMP4 initialization segment
        if let Some(init) = &self.init_segment {
            file.write_all(format!("#EXT-X-MAP:URI=\"{}\"\n", init).as_bytes())
                .await?;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    #[tokio::test]
    async fn test_playlist_metadata() {
        let mut playlist = MediaPlaylist::new(10);
        playlist.playlist_type = Some("VOD".to_string());
        playlist.independent_segments = true;
        playlist.add_segment(9.5, "segment_0.ts".to_string());

        let dir = std::env::temp_dir();
        let path = dir.join("test_playlist.m3u8");

        playlist.write_to(&path).await.unwrap();

        let content = fs::read_to_string(&path).await.unwrap();

        assert!(content.contains("#EXT-X-PLAYLIST-TYPE:VOD"));
        assert!(content.contains("#EXT-X-INDEPENDENT-SEGMENTS"));
        assert!(content.contains("#EXT-X-TARGETDURATION:10"));
        assert!(content.contains("#EXTINF:9.500000,"));
        assert!(content.contains("segment_0.ts"));

        let _ = fs::remove_file(path).await;
    }
}

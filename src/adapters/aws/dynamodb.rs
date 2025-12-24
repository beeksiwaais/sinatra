use crate::domain::jobs::VideoStatus;
use crate::ports::repository::VideoStateRepository;
use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use std::error::Error;

/// DynamoAdapter implements VideoStateRepository for AWS DynamoDB.
#[derive(Clone)]
pub struct DynamoAdapter {
    client: Client,
    table_name: String,
}

impl DynamoAdapter {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }
}

#[async_trait]
impl VideoStateRepository for DynamoAdapter {
    async fn save_video_status(
        &self,
        status: &VideoStatus,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let segment_durations_json = serde_json::to_string(&status.segment_durations)?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .item("video_id", AttributeValue::S(status.id.clone()))
            .item(
                "source_path",
                AttributeValue::S(status.source_path.to_string_lossy().to_string()),
            )
            .item(
                "hls_dir",
                AttributeValue::S(status.hls_dir.to_string_lossy().to_string()),
            )
            .item(
                "total_segments",
                AttributeValue::N(status.total_segments.to_string()),
            )
            .item("completed_segments", AttributeValue::N("0".to_string()))
            .item(
                "segment_durations",
                AttributeValue::S(segment_durations_json),
            )
            .send()
            .await?;
        Ok(())
    }

    async fn get_video_status(
        &self,
        video_id: &str,
    ) -> Result<Option<VideoStatus>, Box<dyn Error + Send + Sync>> {
        let resp = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("video_id", AttributeValue::S(video_id.to_string()))
            .send()
            .await?;

        if let Some(item) = resp.item {
            let id = item
                .get("video_id")
                .and_then(|v| v.as_s().ok())
                .cloned()
                .unwrap_or_default();
            let source_path = item
                .get("source_path")
                .and_then(|v| v.as_s().ok())
                .map(|s| std::path::PathBuf::from(s))
                .unwrap_or_default();
            let hls_dir = item
                .get("hls_dir")
                .and_then(|v| v.as_s().ok())
                .map(|s| std::path::PathBuf::from(s))
                .unwrap_or_default();
            let total_segments = item
                .get("total_segments")
                .and_then(|v| v.as_n().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let segment_durations: Vec<f64> = item
                .get("segment_durations")
                .and_then(|v| v.as_s().ok())
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            Ok(Some(VideoStatus {
                id,
                source_path,
                hls_dir,
                total_segments,
                segment_durations,
            }))
        } else {
            Ok(None)
        }
    }

    async fn mark_segment_complete(
        &self,
        video_id: &str,
    ) -> Result<usize, Box<dyn Error + Send + Sync>> {
        let resp = self
            .client
            .update_item()
            .table_name(&self.table_name)
            .key("video_id", AttributeValue::S(video_id.to_string()))
            .update_expression("SET completed_segments = completed_segments + :inc")
            .expression_attribute_values(":inc", AttributeValue::N("1".to_string()))
            .return_values(aws_sdk_dynamodb::types::ReturnValue::UpdatedNew)
            .send()
            .await?;

        let completed = resp
            .attributes
            .and_then(|attrs| attrs.get("completed_segments").cloned())
            .and_then(|v| v.as_n().ok().cloned())
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        Ok(completed)
    }

    async fn get_total_segments(
        &self,
        video_id: &str,
    ) -> Result<usize, Box<dyn Error + Send + Sync>> {
        let status = self.get_video_status(video_id).await?;
        Ok(status.map(|s| s.total_segments).unwrap_or(0))
    }

    async fn cleanup_video(&self, video_id: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.client
            .delete_item()
            .table_name(&self.table_name)
            .key("video_id", AttributeValue::S(video_id.to_string()))
            .send()
            .await?;
        Ok(())
    }
}

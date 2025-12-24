use crate::domain::jobs::Job;
use crate::ports::queue::JobQueuePort;
use async_trait::async_trait;
use aws_sdk_sqs::Client;
use std::error::Error;

/// SqsAdapter implements JobQueuePort for AWS SQS.
#[derive(Clone)]
pub struct SqsAdapter {
    client: Client,
    queue_url: String,
}

impl SqsAdapter {
    pub fn new(client: Client, queue_url: String) -> Self {
        Self { client, queue_url }
    }
}

#[async_trait]
impl JobQueuePort for SqsAdapter {
    async fn enqueue_job(&self, job: Job) -> Result<(), Box<dyn Error + Send + Sync>> {
        let message_body = serde_json::to_string(&job)?;
        self.client
            .send_message()
            .queue_url(&self.queue_url)
            .message_body(message_body)
            .send()
            .await?;
        Ok(())
    }

    async fn dequeue_job(
        &self,
        timeout_secs: f64,
    ) -> Result<Option<Job>, Box<dyn Error + Send + Sync>> {
        let wait_time = timeout_secs.ceil() as i32;
        let resp = self
            .client
            .receive_message()
            .queue_url(&self.queue_url)
            .max_number_of_messages(1)
            .wait_time_seconds(wait_time)
            .send()
            .await?;

        if let Some(messages) = resp.messages {
            if let Some(msg) = messages.into_iter().next() {
                if let Some(body) = msg.body() {
                    let job: Job = serde_json::from_str(body)?;

                    // Delete the message from the queue after successful processing
                    if let Some(receipt_handle) = msg.receipt_handle() {
                        self.client
                            .delete_message()
                            .queue_url(&self.queue_url)
                            .receipt_handle(receipt_handle)
                            .send()
                            .await?;
                    }
                    return Ok(Some(job));
                }
            }
        }
        Ok(None)
    }
}

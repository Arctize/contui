//! Log streaming operations

use bollard::query_parameters::LogsOptionsBuilder;
use futures::StreamExt;
use std::time::Duration;
use tokio::time::timeout;
use tracing::debug;

use crate::core::{DockerError, Result};
use crate::docker::DockerClient;

/// Timeout for individual log stream items
const LOG_ITEM_TIMEOUT: Duration = Duration::from_secs(2);

/// Log entry from a container
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub message: String,
    pub is_stderr: bool,
}

impl DockerClient {
    /// Fetch the last N lines of logs from a container (non-streaming)
    pub async fn fetch_logs(&self, id: &str, tail: usize) -> Result<Vec<LogEntry>> {
        debug!("Fetching last {} log lines for container {}", tail, id);

        // Build options - enable timestamps for time filtering
        let tail_str = if tail == 0 {
            "all".to_string()
        } else {
            tail.to_string()
        };
        let options = LogsOptionsBuilder::new()
            .stdout(true)
            .stderr(true)
            .timestamps(true) // Enable timestamps for time filtering
            .follow(false)
            .tail(&tail_str)
            .build();

        let mut stream = self.inner().logs(id, Some(options));
        let mut entries = Vec::new();

        // Read logs with timeout on each item to prevent hanging
        loop {
            match timeout(LOG_ITEM_TIMEOUT, stream.next()).await {
                Ok(Some(Ok(log))) => {
                    debug!("Raw log output: {:?}", log);
                    match Self::parse_log_entry(log) {
                        Ok(entry) => {
                            debug!("Parsed log entry: {:?}", entry);
                            entries.push(entry);
                            if entries.len() >= tail {
                                break; // Got enough entries
                            }
                        }
                        Err(e) => {
                            debug!("Failed to parse log entry: {}", e);
                        }
                    }
                }
                Ok(Some(Err(e))) => {
                    debug!("Error reading log: {}", e);
                    // Continue reading other logs
                }
                Ok(None) => {
                    debug!("Log stream ended");
                    break; // Stream ended
                }
                Err(_) => {
                    debug!(
                        "Timeout waiting for next log entry, returning {} entries",
                        entries.len()
                    );
                    break; // Timeout on individual item
                }
            }
        }

        debug!("Fetched {} log entries total", entries.len());
        Ok(entries)
    }

    /// Stream logs from a container
    pub fn stream_logs(
        &self,
        id: &str,
        follow: bool,
        tail: usize,
    ) -> impl futures::Stream<Item = Result<LogEntry>> + '_ {
        use crate::core::ContuiError;

        let options = LogsOptionsBuilder::new()
            .stdout(true)
            .stderr(true)
            .timestamps(true)
            .follow(follow)
            .tail(&tail.to_string())
            .build();

        let stream = self.inner().logs(id, Some(options));

        stream.map(|result| {
            result
                .map_err(|e| {
                    ContuiError::Docker(DockerError::Container(format!(
                        "Failed to read logs: {}",
                        e
                    )))
                })
                .and_then(Self::parse_log_entry)
        })
    }

    /// Parse a log entry from bollard
    fn parse_log_entry(log: bollard::container::LogOutput) -> crate::core::Result<LogEntry> {
        use crate::core::{ContuiError, DockerError};

        // Extract message from log output
        // Note: bollard returns Console for both stdout and stderr when timestamps are enabled
        let raw_message = match log {
            bollard::container::LogOutput::StdOut { message } => {
                String::from_utf8_lossy(&message).to_string()
            }
            bollard::container::LogOutput::StdErr { message } => {
                String::from_utf8_lossy(&message).to_string()
            }
            bollard::container::LogOutput::Console { message } => {
                String::from_utf8_lossy(&message).to_string()
            }
            _ => {
                return Err(ContuiError::Docker(DockerError::Container(
                    "Unknown log output type".to_string(),
                )));
            }
        };

        // Try to detect stderr by content (ERROR, CRITICAL, etc.)
        let is_stderr = raw_message.contains(" ERROR:") || raw_message.contains(" CRITICAL:");

        // Trim the message to remove trailing newlines
        let message = raw_message.trim_end().to_string();

        debug!("Parsing log message: '{}' (len={})", message, message.len());

        // Parse timestamp from message (format: "2024-01-28T10:30:00.123456789Z message")
        // Docker adds timestamps when timestamps=true in options
        // When timestamps=false, we just use the current time
        let (timestamp, message) = if message.len() > 20 {
            // Look for RFC3339 timestamp pattern: YYYY-MM-DDTHH:MM:SS
            if message.chars().nth(4) == Some('-')
                && message.chars().nth(7) == Some('-')
                && message.chars().nth(10) == Some('T')
                && message.chars().nth(13) == Some(':')
            {
                if let Some(pos) = message.find(' ') {
                    let ts_str = &message[..pos];
                    let msg = message[pos + 1..].to_string();
                    let timestamp = chrono::DateTime::parse_from_rfc3339(ts_str)
                        .ok()
                        .map(|dt| dt.with_timezone(&chrono::Utc));
                    debug!("Parsed timestamp: {:?}", timestamp);
                    (timestamp, msg)
                } else {
                    (None, message)
                }
            } else {
                (None, message)
            }
        } else {
            (None, message)
        };

        Ok(LogEntry {
            timestamp,
            message,
            is_stderr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require Docker to be running

    #[tokio::test]
    #[ignore = "requires Docker daemon"]
    async fn test_fetch_logs() {
        let client = DockerClient::from_env().await.unwrap();

        // List containers and try to get logs from the first one
        let containers = client.list_containers(true).await.unwrap();
        if let Some(container) = containers.first() {
            println!(
                "Fetching logs for container: {} ({})",
                container.short_id, container.id
            );
            let logs = client.fetch_logs(&container.id, 10).await;
            println!("Result: {:?}", logs);
            assert!(logs.is_ok());
            let entries = logs.unwrap();
            println!("Fetched {} log entries", entries.len());
            for entry in &entries {
                println!("Log: {:?}", entry);
            }
        } else {
            println!("No containers found to test log fetching");
        }
    }

    #[tokio::test]
    #[ignore = "requires Docker daemon"]
    async fn test_fetch_logs_test_logger() {
        use bollard::query_parameters::LogsOptionsBuilder;
        use futures::StreamExt;

        let client = DockerClient::from_env().await.unwrap();

        // Try to find test-logger container
        let containers = client.list_containers(true).await.unwrap();
        let test_logger = containers
            .iter()
            .find(|c| c.names.iter().any(|n| n.contains("test-logger")));

        if let Some(container) = test_logger {
            println!(
                "Found test-logger: {} ({})",
                container.short_id, container.id
            );

            // Direct bollard call
            let options = LogsOptionsBuilder::new()
                .stdout(true)
                .stderr(true)
                .timestamps(true)
                .follow(false)
                .tail("10")
                .build();

            println!("Calling logs API with options: {:?}", options);
            let mut stream = client.inner().logs(&container.id, Some(options));

            let mut count = 0;
            while let Some(result) = stream.next().await {
                count += 1;
                match result {
                    Ok(log) => {
                        println!("Raw log output {}: {:?}", count, log);
                    }
                    Err(e) => {
                        println!("Error reading log {}: {}", count, e);
                    }
                }
            }
            println!("Total log items received: {}", count);

            // Now try our wrapper
            let logs = client.fetch_logs(&container.id, 10).await;
            println!("fetch_logs result: {:?}", logs);
            if let Ok(entries) = &logs {
                println!("fetch_logs fetched {} entries", entries.len());
            }
        } else {
            println!("test-logger container not found, skipping test");
        }
    }
}

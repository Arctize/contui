//! Docker container stats

use bollard::query_parameters::StatsOptionsBuilder;
use futures::StreamExt;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error};

use crate::core::{ContuiError, DockerError, Result};
use crate::docker::DockerClient;

/// Container stats entry
#[derive(Debug, Clone)]
pub struct StatsEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub cpu_percent: f64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub memory_percent: f64,
    pub network_rx: u64,
    pub network_tx: u64,
    pub block_read: u64,
    pub block_write: u64,
    pub pids: u64,
}

impl DockerClient {
    /// Stream stats from a container
    pub fn stream_stats(&self, id: &str) -> impl futures::Stream<Item = Result<StatsEntry>> + '_ {
        let options = StatsOptionsBuilder::new().stream(true).build();

        let stream = self.inner().stats(id, Some(options));

        stream.map(|result| {
            result
                .map_err(|e| {
                    ContuiError::Docker(DockerError::Container(format!(
                        "Failed to read stats: {}",
                        e
                    )))
                })
                .and_then(Self::parse_stats_entry)
        })
    }

    /// Fetch a single stats snapshot from a container
    pub async fn fetch_stats(&self, id: &str) -> Result<StatsEntry> {
        debug!("Fetching stats for container {}", id);

        let options = StatsOptionsBuilder::new().stream(false).build();

        let mut stream = self.inner().stats(id, Some(options));

        // Read first stats entry with timeout
        match timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(Ok(stats))) => {
                debug!("Received stats for container {}", id);
                Self::parse_stats_entry(stats)
            }
            Ok(Some(Err(e))) => {
                error!("Error reading stats for container {}: {}", id, e);
                Err(ContuiError::Docker(DockerError::Container(format!(
                    "Failed to read stats: {}",
                    e
                ))))
            }
            Ok(None) => {
                error!("Stats stream ended unexpectedly for container {}", id);
                Err(ContuiError::Docker(DockerError::Container(
                    "Stats stream ended unexpectedly".to_string(),
                )))
            }
            Err(_) => {
                error!("Timeout waiting for stats for container {}", id);
                Err(ContuiError::Docker(DockerError::Container(
                    "Timeout waiting for stats".to_string(),
                )))
            }
        }
    }

    /// Parse bollard stats into StatsEntry
    fn parse_stats_entry(stats: bollard::models::ContainerStatsResponse) -> Result<StatsEntry> {
        let timestamp = chrono::Utc::now();

        // Calculate CPU percentage
        let cpu_percent = match (&stats.cpu_stats, &stats.precpu_stats) {
            (Some(cpu), Some(precpu)) => calculate_cpu_percent(cpu, precpu),
            _ => 0.0,
        };

        // Memory stats
        let memory_usage = stats
            .memory_stats
            .as_ref()
            .and_then(|m| m.usage)
            .unwrap_or(0);
        let memory_limit = stats
            .memory_stats
            .as_ref()
            .and_then(|m| m.limit)
            .unwrap_or(1);
        let memory_percent = if memory_limit > 0 {
            (memory_usage as f64 / memory_limit as f64) * 100.0
        } else {
            0.0
        };

        // Network stats (sum all interfaces)
        let (network_rx, network_tx) = stats
            .networks
            .as_ref()
            .map(|networks| {
                networks.values().fold((0u64, 0u64), |(rx, tx), net| {
                    (
                        rx + net.rx_bytes.unwrap_or(0),
                        tx + net.tx_bytes.unwrap_or(0),
                    )
                })
            })
            .unwrap_or((0, 0));

        // Block I/O stats
        let (block_read, block_write) = stats
            .blkio_stats
            .as_ref()
            .and_then(|b| b.io_service_bytes_recursive.as_ref())
            .map(|io_stats| {
                io_stats.iter().fold((0u64, 0u64), |(read, write), entry| {
                    let value = entry.value.unwrap_or(0);
                    match entry.op.as_deref().unwrap_or("") {
                        "Read" => (read + value, write),
                        "Write" => (read, write + value),
                        _ => (read, write),
                    }
                })
            })
            .unwrap_or((0, 0));

        // PIDs
        let pids = stats
            .pids_stats
            .as_ref()
            .and_then(|p| p.current)
            .unwrap_or(0);

        Ok(StatsEntry {
            timestamp,
            cpu_percent,
            memory_usage,
            memory_limit,
            memory_percent,
            network_rx,
            network_tx,
            block_read,
            block_write,
            pids,
        })
    }
}

/// Calculate CPU percentage from current and previous CPU stats
fn calculate_cpu_percent(
    cpu_stats: &bollard::models::ContainerCpuStats,
    precpu_stats: &bollard::models::ContainerCpuStats,
) -> f64 {
    let total = |s: &bollard::models::ContainerCpuStats| {
        s.cpu_usage
            .as_ref()
            .and_then(|u| u.total_usage)
            .unwrap_or(0)
    };
    let cpu_delta = total(cpu_stats).saturating_sub(total(precpu_stats));

    let system_delta = match (cpu_stats.system_cpu_usage, precpu_stats.system_cpu_usage) {
        (Some(curr), Some(prev)) => curr.saturating_sub(prev),
        _ => 0,
    };

    if system_delta > 0 && cpu_delta > 0 {
        let online_cpus = cpu_stats.online_cpus.unwrap_or(1).max(1);
        (cpu_delta as f64 / system_delta as f64) * online_cpus as f64 * 100.0
    } else {
        0.0
    }
}

/// Format bytes to human readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let exp = (bytes as f64).log(1024.0).min(UNITS.len() as f64 - 1.0) as usize;
    let value = bytes as f64 / 1024f64.powi(exp as i32);
    if exp == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[exp])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }
}

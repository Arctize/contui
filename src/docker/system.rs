//! Docker system operations (disk usage, prune)

use bollard::models::SystemDataUsageResponse;
use bollard::query_parameters::PruneContainersOptionsBuilder;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Disk usage information for a specific resource type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Total size in bytes
    pub total: i64,
    /// Reclaimable size in bytes (can be freed)
    pub reclaimable: i64,
    /// Number of items
    pub count: i64,
}

impl ResourceUsage {
    /// Create empty usage
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there's any usage
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
}

/// System-wide disk usage information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemDiskUsage {
    /// Image usage
    pub images: ResourceUsage,
    /// Container usage
    pub containers: ResourceUsage,
    /// Volume usage
    pub volumes: ResourceUsage,
    /// Build cache usage
    pub build_cache: ResourceUsage,
}

impl SystemDiskUsage {
    /// Get total size across all categories
    pub fn total_size(&self) -> i64 {
        self.images.total + self.containers.total + self.volumes.total + self.build_cache.total
    }

    /// Get total reclaimable size across all categories
    pub fn total_reclaimable(&self) -> i64 {
        self.images.reclaimable
            + self.containers.reclaimable
            + self.volumes.reclaimable
            + self.build_cache.reclaimable
    }

    /// Get reclaimable percentage (0-100)
    pub fn reclaimable_percentage(&self) -> f64 {
        let total = self.total_size();
        if total == 0 {
            0.0
        } else {
            (self.total_reclaimable() as f64 / total as f64) * 100.0
        }
    }
}

/// Convert bytes to human-readable format
pub fn format_bytes_size(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes < 1024 * 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!(
            "{:.2} TB",
            bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0)
        )
    }
}

/// Prune options for different resource types
#[derive(Debug, Clone, Default)]
pub struct PruneOptions {
    /// Prune stopped containers
    pub containers: bool,
    /// Prune dangling images
    pub images: bool,
    /// Prune unused volumes
    pub volumes: bool,
    /// Prune unused networks
    pub networks: bool,
    /// Prune build cache (not available in bollard 0.18)
    pub build_cache: bool,
}

/// Result of a prune operation
#[derive(Debug, Clone, Default)]
pub struct PruneResult {
    /// Space reclaimed in bytes
    pub space_reclaimed: i64,
    /// Items that were deleted
    pub items_deleted: Vec<String>,
}

impl PruneResult {
    /// Merge multiple prune results into one
    pub fn merge(results: Vec<PruneResult>) -> Self {
        let mut total = Self::default();
        for result in results {
            total.space_reclaimed += result.space_reclaimed;
            total.items_deleted.extend(result.items_deleted);
        }
        total
    }
}

/// System information from Docker
#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    pub version: String,
    pub api_version: String,
    pub os: String,
    pub arch: String,
    pub kernel_version: String,
}

use super::client::DockerClient;
use anyhow::Result;

impl DockerClient {
    /// Get system disk usage information
    pub async fn get_disk_usage(&self) -> Result<SystemDiskUsage> {
        let response = self.inner().df(None).await?;
        Ok(parse_disk_usage(response))
    }

    /// Get system information
    pub async fn get_system_info(&self) -> Result<SystemInfo> {
        let version = self.inner().version().await?;
        let info = self.inner().info().await?;

        Ok(SystemInfo {
            version: version.version.unwrap_or_default(),
            api_version: version.api_version.unwrap_or_default(),
            os: version.os.unwrap_or_default(),
            arch: version.arch.unwrap_or_default(),
            kernel_version: info.kernel_version.unwrap_or_default(),
        })
    }

    /// Prune stopped containers (with detailed result)
    pub async fn prune_containers_detailed(&self) -> Result<PruneResult> {
        info!("Calling prune_containers API");
        // Create empty filters to prune all stopped containers
        let filters: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let options = PruneContainersOptionsBuilder::new()
            .filters(&filters)
            .build();
        let response = self.inner().prune_containers(Some(options)).await?;
        info!(
            "Prune containers response: {:?} deleted",
            response
                .containers_deleted
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0)
        );
        Ok(PruneResult {
            space_reclaimed: response.space_reclaimed.unwrap_or(0),
            items_deleted: response.containers_deleted.unwrap_or_default(),
        })
    }
}

/// Parse SystemDataUsageResponse into our SystemDiskUsage.
///
/// The engine (API >= 1.52) reports per-category summaries, so no per-item
/// walk is needed.
fn parse_disk_usage(response: SystemDataUsageResponse) -> SystemDiskUsage {
    fn resource(total: Option<i64>, reclaimable: Option<i64>, count: Option<i64>) -> ResourceUsage {
        ResourceUsage {
            total: total.unwrap_or(0),
            reclaimable: reclaimable.unwrap_or(0),
            count: count.unwrap_or(0),
        }
    }

    let mut usage = SystemDiskUsage::default();
    if let Some(u) = response.image_usage {
        usage.images = resource(u.total_size, u.reclaimable, u.total_count);
    }
    if let Some(u) = response.container_usage {
        usage.containers = resource(u.total_size, u.reclaimable, u.total_count);
    }
    if let Some(u) = response.volume_usage {
        usage.volumes = resource(u.total_size, u.reclaimable, u.total_count);
    }
    if let Some(u) = response.build_cache_usage {
        usage.build_cache = resource(u.total_size, u.reclaimable, u.total_count);
    }
    usage
}

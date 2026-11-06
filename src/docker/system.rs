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
/// Docker (API >= 1.52) reports per-category summaries. Engines that only
/// list the items (such as Podman) get totals derived from those items.
fn parse_disk_usage(response: SystemDataUsageResponse) -> SystemDiskUsage {
    use serde_json::Value;

    fn int(item: &Value, key: &str) -> i64 {
        item.get(key).and_then(Value::as_i64).unwrap_or(0)
    }

    /// Sum `size(item)` over all items, and over the items where `reclaim(item)`.
    fn sums(
        items: &[Value],
        size: impl Fn(&Value) -> i64,
        reclaim: impl Fn(&Value) -> bool,
    ) -> (i64, i64) {
        items.iter().fold((0, 0), |(total, reclaimable), item| {
            let n = size(item);
            (total + n, reclaimable + if reclaim(item) { n } else { 0 })
        })
    }

    fn resource(
        summary: (Option<i64>, Option<i64>, Option<i64>), // total, reclaimable, count
        items: Option<&Vec<Value>>,
        derive: impl Fn(&[Value]) -> (i64, i64),
    ) -> ResourceUsage {
        let items = items.map(Vec::as_slice).unwrap_or(&[]);
        let (total, reclaimable, count) = summary;
        let derived = if total.is_none() || reclaimable.is_none() {
            derive(items)
        } else {
            (0, 0)
        };
        ResourceUsage {
            total: total.unwrap_or(derived.0),
            reclaimable: reclaimable
                .unwrap_or(derived.1)
                .min(total.unwrap_or(derived.0)),
            count: count.unwrap_or(items.len() as i64),
        }
    }

    let mut usage = SystemDiskUsage::default();
    if let Some(u) = response.image_usage {
        usage.images = resource(
            (u.total_size, u.reclaimable, u.total_count),
            u.items.as_ref(),
            // untagged images that no container uses are reclaimable. Image
            // sizes overlap through shared layers, so be conservative.
            |items| {
                sums(
                    items,
                    |i| int(i, "Size"),
                    |i| {
                        int(i, "Containers") == 0
                            && i.get("RepoTags")
                                .and_then(Value::as_array)
                                .map_or(true, |t| t.is_empty())
                    },
                )
            },
        );
    }
    if let Some(u) = response.container_usage {
        usage.containers = resource(
            (u.total_size, u.reclaimable, u.total_count),
            u.items.as_ref(),
            // stopped containers are reclaimable
            |items| {
                sums(
                    items,
                    |i| int(i, "SizeRw"),
                    |i| i.get("State").and_then(Value::as_str) != Some("running"),
                )
            },
        );
    }
    if let Some(u) = response.volume_usage {
        usage.volumes = resource(
            (u.total_size, u.reclaimable, u.total_count),
            u.items.as_ref(),
            // volumes without references are reclaimable; size -1 means unknown
            |items| {
                let usage_int = |i: &Value, key: &str| {
                    i.get("UsageData")
                        .and_then(|d| d.get(key))
                        .and_then(Value::as_i64)
                        .unwrap_or(-1)
                };
                sums(
                    items,
                    |i| usage_int(i, "Size").max(0),
                    |i| usage_int(i, "RefCount") == 0,
                )
            },
        );
    }
    if let Some(u) = response.build_cache_usage {
        usage.build_cache = resource(
            (u.total_size, u.reclaimable, u.total_count),
            u.items.as_ref(),
            |items| {
                sums(
                    items,
                    |i| int(i, "Size"),
                    |i| i.get("InUse").and_then(Value::as_bool) == Some(false),
                )
            },
        );
    }
    usage
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(v: serde_json::Value) -> SystemDiskUsage {
        parse_disk_usage(serde_json::from_value(v).unwrap())
    }

    #[test]
    fn uses_engine_summaries() {
        let u = parse(json!({
            "ImageUsage": {"TotalSize": 100, "Reclaimable": 40, "TotalCount": 3},
            "ContainerUsage": {"TotalSize": 10, "Reclaimable": 10, "TotalCount": 2},
            "VolumeUsage": {"TotalSize": 7, "Reclaimable": 5, "TotalCount": 1},
            "BuildCacheUsage": {"TotalSize": 9, "Reclaimable": 8, "TotalCount": 4},
        }));
        assert_eq!(
            (u.images.total, u.images.reclaimable, u.images.count),
            (100, 40, 3)
        );
        assert_eq!((u.containers.total, u.containers.count), (10, 2));
        assert_eq!((u.volumes.total, u.volumes.reclaimable), (7, 5));
        assert_eq!((u.build_cache.total, u.build_cache.reclaimable), (9, 8));
        assert_eq!(u.total_reclaimable(), 63);
    }

    #[test]
    fn derives_totals_from_items_when_no_summary() {
        // Shape reported by Podman: items only, plus an image total
        let u = parse(json!({
            "ImageUsage": {"TotalSize": 500, "Items": [
                {"Size": 300, "Containers": 0, "RepoTags": null},
                {"Size": 400, "Containers": 0, "RepoTags": ["a:1"]},
                {"Size": 200, "Containers": 1, "RepoTags": []}]},
            "ContainerUsage": {"Items": [
                {"SizeRw": 5, "State": "running"}, {"SizeRw": 7, "State": "exited"}]},
            "VolumeUsage": {"Items": [
                {"UsageData": {"Size": 50, "RefCount": 0}},
                {"UsageData": {"Size": 30, "RefCount": 2}},
                {"UsageData": {"Size": -1, "RefCount": -1}}]},
            "BuildCacheUsage": {"Items": []},
        }));
        assert_eq!(u.images.total, 500);
        assert_eq!(u.images.reclaimable, 300);
        assert_eq!(u.images.count, 3);
        assert_eq!(
            (
                u.containers.total,
                u.containers.reclaimable,
                u.containers.count
            ),
            (12, 7, 2)
        );
        assert_eq!(
            (u.volumes.total, u.volumes.reclaimable, u.volumes.count),
            (80, 50, 3)
        );
        assert_eq!(u.build_cache.total, 0);
    }

    #[test]
    fn empty_response_is_zero() {
        let u = parse(json!({}));
        assert_eq!(u.total_size(), 0);
    }
}

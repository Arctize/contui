//! Docker image inspection

use crate::core::{ContuiError, DockerError, Result};
use crate::docker::DockerClient;

/// Image details from inspect
#[derive(Debug, Clone)]
pub struct ImageDetails {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size: u64,
    pub created: String,
    pub author: String,
    pub os: String,
    pub architecture: String,
    pub exposed_ports: Vec<String>,
    pub env: Vec<String>,
    pub entrypoint: Option<Vec<String>>,
    pub cmd: Option<Vec<String>>,
    pub labels: Vec<(String, String)>,
    pub layers: Vec<ImageLayer>,
}

#[derive(Debug, Clone)]
pub struct ImageLayer {
    pub id: String,
    pub created: String,
    pub created_by: String,
    pub size: i64,
    pub comment: String,
}

impl DockerClient {
    /// Inspect an image and get detailed information
    pub async fn inspect_image(&self, id: &str) -> Result<ImageDetails> {
        use tracing::debug;

        debug!("Inspecting image {}", id);

        let inspect = self.inner().inspect_image(id).await.map_err(|e| {
            ContuiError::Docker(DockerError::Image(format!("Failed to inspect: {}", e)))
        })?;

        // Get history for layers
        let history = self.inner().image_history(id).await.map_err(|e| {
            ContuiError::Docker(DockerError::Image(format!("Failed to get history: {}", e)))
        })?;

        // Extract repo tags
        let repo_tags = inspect.repo_tags.unwrap_or_default();

        // Extract config info
        let config = inspect.config;

        // Extract exposed ports
        let exposed_ports = config
            .as_ref()
            .and_then(|c| c.exposed_ports.clone())
            .map(|ports| ports.into_iter().collect())
            .unwrap_or_default();

        // Extract env vars
        let env = config
            .as_ref()
            .and_then(|c| c.env.clone())
            .unwrap_or_default();

        // Extract entrypoint
        let entrypoint = config.as_ref().and_then(|c| c.entrypoint.clone());

        // Extract command
        let cmd = config.as_ref().and_then(|c| c.cmd.clone());

        // Extract labels
        let labels = config
            .as_ref()
            .and_then(|c| c.labels.clone())
            .map(|l| l.into_iter().collect::<Vec<_>>())
            .unwrap_or_default();

        // Parse layers from history
        let layers: Vec<ImageLayer> = history
            .into_iter()
            .map(|h| {
                ImageLayer {
                    id: h.id,
                    created: format!("{}", h.created),
                    created_by: h.created_by,
                    size: h.size,
                    comment: String::new(), // comment field doesn't exist in HistoryResponseItem
                }
            })
            .collect();

        Ok(ImageDetails {
            id: inspect.id.unwrap_or_default(),
            repo_tags,
            size: inspect.size.unwrap_or(0) as u64,
            created: inspect.created.unwrap_or_default(),
            author: inspect.author.unwrap_or_default(),
            os: inspect.os.unwrap_or_default(),
            architecture: inspect.architecture.unwrap_or_default(),
            exposed_ports,
            env,
            entrypoint,
            cmd,
            labels,
            layers,
        })
    }
}

/// Format size in human readable format
pub fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if size == 0 {
        return "0 B".to_string();
    }
    let exp = (size as f64).log(1024.0).min(UNITS.len() as f64 - 1.0) as usize;
    let value = size as f64 / 1024f64.powi(exp as i32);
    if exp == 0 {
        format!("{} {}", size, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[exp])
    }
}

/// Format signed size (for layers which can be negative)
pub fn format_signed_size(size: i64) -> String {
    if size < 0 {
        format!("- {}", format_size((-size) as u64))
    } else {
        format_size(size as u64)
    }
}

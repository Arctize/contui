//! Image operations

use bollard::query_parameters::{
    ListImagesOptionsBuilder, PruneImagesOptionsBuilder, RemoveImageOptionsBuilder,
};
use tracing::{debug, info};

use crate::core::{DockerError, ImageSummary, Result};
use crate::docker::DockerClient;

impl DockerClient {
    /// List all images
    pub async fn list_images(&self, all: bool) -> Result<Vec<ImageSummary>> {
        debug!("Listing images (all={})", all);

        let options = ListImagesOptionsBuilder::new().all(all).build();

        let images = self
            .inner()
            .list_images(Some(options))
            .await
            .map_err(|e| DockerError::Image(e.to_string()))?;

        info!("Found {} images", images.len());

        Ok(images.into_iter().map(|i| i.into()).collect())
    }

    /// Remove an image
    pub async fn remove_image(&self, id: &str, force: bool) -> Result<()> {
        info!("Removing image: {} (force={})", id, force);

        let options = RemoveImageOptionsBuilder::new().force(force).build();

        self.inner()
            .remove_image(id, Some(options), None)
            .await
            .map_err(|e| DockerError::Image(format!("Failed to remove {}: {}", id, e)))?;

        info!("Image {} removed successfully", id);
        Ok(())
    }

    /// Prune dangling images (untagged images)
    pub async fn prune_images(&self) -> Result<u64> {
        info!("Pruning dangling images");

        let filters =
            std::collections::HashMap::from([("dangling".to_string(), vec!["true".to_string()])]);

        let result = self
            .inner()
            .prune_images(Some(
                PruneImagesOptionsBuilder::new().filters(&filters).build(),
            ))
            .await
            .map_err(|e| DockerError::Image(format!("Failed to prune images: {}", e)))?;

        let reclaimed = result.space_reclaimed.unwrap_or(0) as u64;
        info!("Pruned images, reclaimed {} bytes", reclaimed);
        Ok(reclaimed)
    }
}

impl From<bollard::models::ImageSummary> for ImageSummary {
    fn from(i: bollard::models::ImageSummary) -> Self {
        let id = i.id.clone();
        let short_id = id.chars().take(12).collect();

        // Determine if dangling (no repo tags or <none>:<none>)
        let dangling = i.repo_tags.is_empty() || i.repo_tags.iter().all(|t| t.contains("<none>"));

        Self {
            id,
            short_id,
            repo_tags: i.repo_tags.clone(),
            repo_digests: i.repo_digests.clone(),
            created: chrono::DateTime::from_timestamp(i.created, 0).unwrap_or(chrono::Utc::now()),
            size: i.size,
            shared_size: i.shared_size,
            virtual_size: i.size,
            labels: i.labels,
            containers: i.containers as i32,
            dangling,
            parent_id: i.parent_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require Docker to be running

    #[tokio::test]
    #[ignore = "requires Docker daemon"]
    async fn test_list_images() {
        let client = DockerClient::from_env().await.unwrap();
        let images = client.list_images(true).await;
        assert!(images.is_ok());
    }
}

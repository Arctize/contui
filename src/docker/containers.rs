//! Container operations

use bollard::query_parameters::{
    KillContainerOptionsBuilder, ListContainersOptionsBuilder, RemoveContainerOptionsBuilder,
    RestartContainerOptionsBuilder, StopContainerOptionsBuilder,
};
use tracing::{debug, info, warn};

use crate::core::{ContainerState, ContainerSummary, DockerError, Result};
use crate::docker::DockerClient;

impl DockerClient {
    /// List all containers
    pub async fn list_containers(&self, all: bool) -> Result<Vec<ContainerSummary>> {
        debug!("Listing containers (all={})", all);

        let options = ListContainersOptionsBuilder::new().all(all).build();

        let containers = self
            .inner()
            .list_containers(Some(options))
            .await
            .map_err(|e: bollard::errors::Error| DockerError::Container(e.to_string()))?;

        info!("Found {} containers", containers.len());

        Ok(containers
            .into_iter()
            .map(|c: bollard::models::ContainerSummary| c.into())
            .collect())
    }

    /// Start a container
    pub async fn start_container(&self, id: &str) -> Result<()> {
        info!("Starting container: {}", id);

        self.inner()
            .start_container(id, None)
            .await
            .map_err(|e: bollard::errors::Error| {
                DockerError::Container(format!("Failed to start {}: {}", id, e))
            })?;

        info!("Container {} started successfully", id);
        Ok(())
    }

    /// Stop a container
    pub async fn stop_container(&self, id: &str, timeout: Option<i64>) -> Result<()> {
        let timeout = timeout.unwrap_or(10);
        info!("Stopping container: {} (timeout={}s)", id, timeout);

        let options = StopContainerOptionsBuilder::new().t(timeout as i32).build();

        self.inner()
            .stop_container(id, Some(options))
            .await
            .map_err(|e: bollard::errors::Error| {
                DockerError::Container(format!("Failed to stop {}: {}", id, e))
            })?;

        info!("Container {} stopped successfully", id);
        Ok(())
    }

    /// Restart a container
    pub async fn restart_container(&self, id: &str, timeout: Option<isize>) -> Result<()> {
        let timeout = timeout.unwrap_or(10);
        info!("Restarting container: {} (timeout={}s)", id, timeout);

        let options = RestartContainerOptionsBuilder::new()
            .t(timeout as i32)
            .build();

        self.inner()
            .restart_container(id, Some(options))
            .await
            .map_err(|e: bollard::errors::Error| {
                DockerError::Container(format!("Failed to restart {}: {}", id, e))
            })?;

        info!("Container {} restarted successfully", id);
        Ok(())
    }

    /// Pause a container
    pub async fn pause_container(&self, id: &str) -> Result<()> {
        info!("Pausing container: {}", id);

        self.inner()
            .pause_container(id)
            .await
            .map_err(|e: bollard::errors::Error| {
                DockerError::Container(format!("Failed to pause {}: {}", id, e))
            })?;

        info!("Container {} paused successfully", id);
        Ok(())
    }

    /// Unpause a container
    pub async fn unpause_container(&self, id: &str) -> Result<()> {
        info!("Unpausing container: {}", id);

        self.inner()
            .unpause_container(id)
            .await
            .map_err(|e: bollard::errors::Error| {
                DockerError::Container(format!("Failed to unpause {}: {}", id, e))
            })?;

        info!("Container {} unpaused successfully", id);
        Ok(())
    }

    /// Kill a container
    pub async fn kill_container(&self, id: &str, signal: Option<&str>) -> Result<()> {
        let signal = signal.unwrap_or("SIGKILL");
        warn!("Killing container: {} (signal={})", id, signal);

        let options = KillContainerOptionsBuilder::new().signal(signal).build();

        self.inner()
            .kill_container(id, Some(options))
            .await
            .map_err(|e: bollard::errors::Error| {
                DockerError::Container(format!("Failed to kill {}: {}", id, e))
            })?;

        info!("Container {} killed successfully", id);
        Ok(())
    }

    /// Remove a container
    pub async fn remove_container(
        &self,
        id: &str,
        force: bool,
        remove_volumes: bool,
    ) -> Result<()> {
        warn!(
            "Removing container: {} (force={}, remove_volumes={})",
            id, force, remove_volumes
        );

        let options = RemoveContainerOptionsBuilder::new()
            .v(remove_volumes)
            .force(force)
            .link(false)
            .build();

        self.inner()
            .remove_container(id, Some(options))
            .await
            .map_err(|e: bollard::errors::Error| {
                DockerError::Container(format!("Failed to remove {}: {}", id, e))
            })?;

        info!("Container {} removed successfully", id);
        Ok(())
    }
}

// Conversion implementations
impl From<bollard::models::ContainerSummary> for ContainerSummary {
    fn from(c: bollard::models::ContainerSummary) -> Self {
        let id = c.id.clone().unwrap_or_default();
        let short_id = id.chars().take(12).collect();

        // Parse state from status string
        let state = parse_container_state(c.state.as_ref().map(|s| s.as_ref()));
        let status = c.status.clone().unwrap_or_default();

        // Parse names (remove leading slashes)
        let names: Vec<String> = c
            .names
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|n| n.trim_start_matches('/').to_string())
            .collect();

        // Parse ports
        let ports: Vec<_> = c
            .ports
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| {
                Some(crate::core::PortMapping {
                    ip: p.ip.map(|s| s.to_string()),
                    private_port: p.private_port,
                    public_port: p.public_port.map(|p| p),
                    protocol: p
                        .typ
                        .map(|t| format!("{:?}", t))
                        .unwrap_or_else(|| "tcp".to_string()),
                })
            })
            .collect();

        // Parse labels for compose project/service
        let labels = c.labels.clone().unwrap_or_default();
        let compose_project = labels.get("com.docker.compose.project").cloned();
        let compose_service = labels.get("com.docker.compose.service").cloned();

        Self {
            id,
            short_id,
            names,
            image: c.image.clone().unwrap_or_default(),
            image_id: c.image_id.clone().unwrap_or_default(),
            command: c.command.clone().unwrap_or_default(),
            created: chrono::DateTime::from_timestamp(c.created.unwrap_or(0), 0)
                .unwrap_or_else(chrono::Utc::now),
            ports,
            size_rw: c.size_rw,
            size_root_fs: c.size_root_fs,
            labels,
            state,
            status,
            health: None,   // Will be populated by inspect
            mounts: vec![], // Will be populated by inspect
            networks: c
                .network_settings
                .clone()
                .map(|ns| ns.networks.unwrap_or_default().into_keys().collect())
                .unwrap_or_default(),
            compose_project,
            compose_service,
        }
    }
}

fn parse_container_state(state: Option<&str>) -> ContainerState {
    match state {
        Some("created") => ContainerState::Created,
        Some("running") => ContainerState::Running,
        Some("paused") => ContainerState::Paused,
        Some("restarting") => ContainerState::Restarting,
        Some("removing") => ContainerState::Removing,
        Some("exited") => ContainerState::Exited,
        Some("dead") => ContainerState::Dead,
        _ => ContainerState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_container_state() {
        assert_eq!(
            parse_container_state(Some("running")),
            ContainerState::Running
        );
        assert_eq!(
            parse_container_state(Some("exited")),
            ContainerState::Exited
        );
        assert_eq!(
            parse_container_state(Some("paused")),
            ContainerState::Paused
        );
        assert_eq!(parse_container_state(None), ContainerState::Unknown);
    }

    // Integration tests require Docker daemon
    #[tokio::test]
    #[ignore = "requires Docker daemon"]
    async fn test_list_containers() {
        use crate::docker::DockerClient;
        let client = DockerClient::from_env().await.unwrap();
        let containers = client.list_containers(true).await;
        assert!(containers.is_ok());
    }
}

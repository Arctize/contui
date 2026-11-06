use std::sync::Arc;

use bollard::Docker;
use tracing::{debug, info};

use std::path::{Path, PathBuf};

use crate::config::Runtime;
use crate::core::{ConnectionInfo, DockerError, Result};

/// Docker client wrapper
#[derive(Clone)]
pub struct DockerClient {
    inner: Arc<Docker>,
    connection_info: ConnectionInfo,
}

impl DockerClient {
    /// Create a new client from environment (DOCKER_HOST, etc.)
    pub async fn from_env() -> Result<Self> {
        info!("Creating Docker client from environment");

        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| DockerError::Connection(e.to_string()))?;

        Self::new(docker, "local").await
    }

    /// Connect to the engine selected by `host` and `runtime`.
    ///
    /// An explicit `host` always wins. Without one, the local socket is used:
    /// `docker` and `podman` pick that engine, `auto` prefers docker and falls
    /// back to podman when no docker socket exists.
    pub async fn connect(host: Option<&str>, runtime: Runtime) -> Result<Self> {
        if let Some(host) = host {
            return Self::with_host(host).await;
        }
        match runtime {
            Runtime::Docker => Self::from_env().await,
            Runtime::Podman => Self::podman_local().await,
            Runtime::Auto => {
                if std::env::var_os("DOCKER_HOST").is_some() || docker_socket().is_some() {
                    Self::from_env().await
                } else if podman_socket().is_some() {
                    Self::podman_local().await
                } else {
                    // Nothing found: fail with the usual docker error
                    Self::from_env().await
                }
            }
        }
    }

    /// Connect to the local podman API socket
    async fn podman_local() -> Result<Self> {
        info!("Creating Podman client from local socket");

        let docker = Docker::connect_with_podman_defaults()
            .map_err(|e| DockerError::Connection(e.to_string()))?;

        Self::new(docker, "local (podman)").await
    }

    /// Create a new client with custom host
    pub async fn with_host(host: &str) -> Result<Self> {
        info!("Creating Docker client with host: {}", host);

        let docker = Docker::connect_with_http(host, 120, bollard::API_DEFAULT_VERSION)
            .map_err(|e| DockerError::Connection(e.to_string()))?;

        Self::new(docker, host).await
    }

    /// Internal constructor
    async fn new(docker: Docker, host: &str) -> Result<Self> {
        debug!("Fetching Docker version information");

        let version = docker
            .version()
            .await
            .map_err(|e| DockerError::Connection(e.to_string()))?;

        let info = ConnectionInfo {
            host: host.to_string(), // Docker doesn't expose host URL directly
            version: version.version.unwrap_or_else(|| "unknown".to_string()),
            api_version: version.api_version.unwrap_or_else(|| "unknown".to_string()),
            os: version.os.unwrap_or_else(|| "unknown".to_string()),
            arch: version.arch.unwrap_or_else(|| "unknown".to_string()),
        };

        info!(
            "Docker client initialized: {} (API: {}) on {}/{}",
            info.version, info.api_version, info.os, info.arch
        );

        Ok(Self {
            inner: Arc::new(docker),
            connection_info: info,
        })
    }

    /// Get connection information
    pub fn connection_info(&self) -> &ConnectionInfo {
        &self.connection_info
    }

    /// Ping the Docker daemon
    pub async fn ping(&self) -> Result<String> {
        debug!("Pinging Docker daemon");

        let response = self
            .inner
            .ping()
            .await
            .map_err(|e| DockerError::Connection(e.to_string()))?;

        Ok(response)
    }

    /// Get the inner Docker client (for advanced usage)
    pub fn inner(&self) -> &Docker {
        &self.inner
    }
}

/// Path of the local docker socket, if one exists
fn docker_socket() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("/var/run/docker.sock")];
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        candidates.push(Path::new(&dir).join("docker.sock")); // rootless docker
    }
    candidates.into_iter().find(|p| p.exists())
}

/// Path of the local podman socket, if one exists
fn podman_socket() -> Option<PathBuf> {
    let mut candidates = vec![];
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        candidates.push(Path::new(&dir).join("podman/podman.sock")); // rootless
    }
    candidates.push(PathBuf::from("/run/podman/podman.sock"));
    candidates.into_iter().find(|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require Docker to be running
    // Mark them with #[ignore] for CI environments without Docker

    #[tokio::test]
    #[ignore = "requires Docker daemon"]
    async fn test_from_env() {
        let client = DockerClient::from_env().await;
        assert!(client.is_ok());

        let client = client.unwrap();
        assert!(!client.connection_info().version.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires Docker daemon"]
    async fn test_ping() {
        let client = DockerClient::from_env().await.unwrap();
        let result = client.ping().await;
        assert!(result.is_ok());
    }
}

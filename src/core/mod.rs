use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod compose;
pub mod errors;
pub mod types;

pub use errors::*;
pub use types::{
    new_operation_id, ConfirmDialog, ContainerId, FilterOp, HelpContent, HelpSection, ImageId,
    InputDialog, Modal, NetworkId, NotificationLevel, OperationId, Panel, SortDirection, Tab,
    VolumeName,
};

/// Docker connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub host: String,
    pub version: String,
    pub api_version: String,
    pub os: String,
    pub arch: String,
}

impl Default for ConnectionInfo {
    fn default() -> Self {
        Self {
            host: "unknown".to_string(),
            version: "unknown".to_string(),
            api_version: "unknown".to_string(),
            os: "unknown".to_string(),
            arch: "unknown".to_string(),
        }
    }
}

/// Port mapping information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub ip: Option<String>,
    pub private_port: u16,
    pub public_port: Option<u16>,
    pub protocol: String,
}

/// Health check status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Starting,
    Healthy,
    Unhealthy,
    None,
}

/// Mount point information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPoint {
    pub source: String,
    pub destination: String,
    pub mode: String,
    pub rw: bool,
    pub propagation: String,
    pub typ: MountType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MountType {
    Bind,
    Volume,
    Tmpfs,
    Npipe,
}

/// Container runtime state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Unknown,
}

impl std::fmt::Display for ContainerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ContainerState::Created => "Created",
            ContainerState::Running => "Running",
            ContainerState::Paused => "Paused",
            ContainerState::Restarting => "Restarting",
            ContainerState::Removing => "Removing",
            ContainerState::Exited => "Exited",
            ContainerState::Dead => "Dead",
            ContainerState::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

/// Container summary for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSummary {
    pub id: String,
    pub short_id: String,
    pub names: Vec<String>,
    pub image: String,
    pub image_id: String,
    pub command: String,
    pub created: DateTime<Utc>,
    pub ports: Vec<PortMapping>,
    pub size_rw: Option<i64>,
    pub size_root_fs: Option<i64>,
    pub labels: HashMap<String, String>,
    pub state: ContainerState,
    pub status: String,
    pub health: Option<HealthStatus>,
    pub mounts: Vec<MountPoint>,
    pub networks: Vec<String>,
    pub compose_project: Option<String>,
    pub compose_service: Option<String>,
}

impl Default for ContainerSummary {
    fn default() -> Self {
        Self {
            id: String::new(),
            short_id: String::new(),
            names: vec![],
            image: String::new(),
            image_id: String::new(),
            command: String::new(),
            created: Utc::now(),
            ports: vec![],
            size_rw: None,
            size_root_fs: None,
            labels: HashMap::new(),
            state: ContainerState::Unknown,
            status: String::new(),
            health: None,
            mounts: vec![],
            networks: vec![],
            compose_project: None,
            compose_service: None,
        }
    }
}

/// Image summary for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSummary {
    pub id: String,
    pub short_id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub created: DateTime<Utc>,
    pub size: i64,
    pub shared_size: i64,
    pub virtual_size: i64,
    pub labels: HashMap<String, String>,
    pub containers: i32,
    pub dangling: bool,
    pub parent_id: String,
}

/// Volume summary for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSummary {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created_at: DateTime<Utc>,
    pub status: HashMap<String, String>,
    pub labels: HashMap<String, String>,
    pub scope: VolumeScope,
    pub options: HashMap<String, String>,
    pub in_use: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeScope {
    Local,
    Global,
}

/// Network summary for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSummary {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: NetworkScope,
    pub created: DateTime<Utc>,
    pub internal: bool,
    pub attachable: bool,
    pub ingress: bool,
    pub enable_ipv6: bool,
    pub connected_containers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkScope {
    Local,
    Global,
    Swarm,
}

/// UI Actions that can be triggered from the UI
#[derive(Debug, Clone)]
pub enum UiAction {
    /// No action
    None,
    /// Quit the application
    Quit,
    /// Start a container
    StartContainer(String),
    /// Stop a container
    StopContainer(String),
    /// Restart a container
    RestartContainer(String),
    /// Pause a container
    PauseContainer(String),
    /// Unpause a container
    UnpauseContainer(String),
    /// Kill a container
    KillContainer(String),
    /// Remove a container
    RemoveContainer(String),
    /// Show logs for a container
    ShowContainerLogs(String),
    /// Show stats for a container
    ShowContainerStats(String),
    /// Show details for a container
    ShowContainerDetails(String),
    /// Exec into a container
    ExecContainer(String),
    /// Start container then exec
    StartContainerAndExec(String),
    /// Send input to exec session
    ExecInput(Vec<u8>),
    /// Show details for an image
    ShowImageDetails(String),
    /// Remove an image
    RemoveImage(String),
    /// Prune dangling images
    PruneImages,
    /// Remove a volume
    RemoveVolume(String),
    /// Prune unused volumes
    PruneVolumes,
    /// Remove a network
    RemoveNetwork(String),
    /// Prune unused networks
    PruneNetworks,
    /// Export logs to file
    ExportLogs,
    /// Clear terminal screen (to prevent ghost text)
    Clear,
    /// Prune system resources (containers, images, volumes, networks, build cache)
    PruneSystem,
}

/// Confirmation dialog action
#[derive(Debug, Clone)]
pub struct ConfirmAction {
    pub message: String,
    pub action: UiAction,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_state_display() {
        assert_eq!(ContainerState::Running.to_string(), "Running");
        assert_eq!(ContainerState::Exited.to_string(), "Exited");
    }

    #[test]
    fn test_default_container_summary() {
        let summary = ContainerSummary::default();
        assert_eq!(summary.state, ContainerState::Unknown);
        assert!(summary.names.is_empty());
    }
}

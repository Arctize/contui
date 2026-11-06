//! Main application coordinator

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::core::{
    ConnectionInfo, ContainerSummary, ImageSummary, NetworkSummary, NotificationLevel,
    Result as ContuiResult, VolumeSummary,
};
use crate::docker::exec::ExecStart;
use crate::docker::{select_exec_command, DockerClient, LogEntry, SystemDiskUsage};
use crate::exec::spinner;
use crate::state::AppState;
use crate::ui::{UiAction, UiApp};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// Main application struct
pub struct App {
    #[allow(dead_code)]
    config: Config,
    state: AppState,
    docker_client: Option<DockerClient>,
    /// Channel receiver for log fetch results
    log_fetch_rx: Option<mpsc::Receiver<ContuiResult<Vec<LogEntry>>>>,
    /// Last time we auto-fetched logs (for follow mode)
    last_log_fetch: Option<std::time::Instant>,
    /// Channel receiver for stats fetch results
    stats_fetch_rx: Option<mpsc::Receiver<ContuiResult<crate::docker::StatsEntry>>>,
    /// Last time we auto-fetched stats (for follow mode)
    last_stats_fetch: Option<std::time::Instant>,
    /// Channel receiver for data refresh results
    data_refresh_handle: Option<DataRefreshHandle>,
    /// Gate for data refresh requests
    data_refresh_gate: RefreshGate,
    /// Last time we requested a data refresh
    last_data_refresh: std::time::Instant,
    /// Exec runtime state
    exec_runtime: Option<ExecRuntime>,
    /// Channel receiver for exec start results
    exec_start_rx: Option<mpsc::Receiver<ExecStartResult>>,
    /// Pending exec start metadata (for spinner/status)
    exec_start_pending: Option<ExecStartPending>,
    /// Track last terminal size for exec resize
    last_terminal_size: Option<(u16, u16)>,
}

enum ExecOutput {
    Bytes(Vec<u8>),
    End,
}

struct ExecRuntime {
    exec_id: String,
    container_id: String,
    input: Pin<Box<dyn tokio::io::AsyncWrite + Send>>,
    parser: vt100::Parser,
    output_rx: mpsc::Receiver<ExecOutput>,
    size: (u16, u16),
}

struct ExecStartPending {
    container_id: String,
    spinner_index: usize,
}

enum ExecStartResult {
    Started {
        exec: ExecStart,
        container_id: String,
        container_name: String,
        size: (u16, u16),
    },
    Failed {
        container_id: String,
        message: String,
    },
}

#[derive(Debug, Default)]
struct RefreshGate {
    in_flight: bool,
    pending: bool,
}

impl RefreshGate {
    fn request(&mut self) -> bool {
        if self.in_flight {
            self.pending = true;
            false
        } else {
            self.in_flight = true;
            true
        }
    }

    fn complete(&mut self) -> bool {
        self.in_flight = false;
        if self.pending {
            self.pending = false;
            true
        } else {
            false
        }
    }
}

struct DataRefreshHandle {
    rx: mpsc::Receiver<DataRefreshResult>,
    had_client: bool,
}

#[derive(Debug, Default)]
struct DataRefreshData {
    containers: Option<Vec<ContainerSummary>>,
    images: Option<Vec<ImageSummary>>,
    volumes: Option<Vec<VolumeSummary>>,
    networks: Option<Vec<NetworkSummary>>,
    disk_usage: Option<SystemDiskUsage>,
}

impl DataRefreshData {
    async fn fetch(client: &DockerClient) -> Self {
        let containers = match client.list_containers(true).await {
            Ok(containers) => {
                debug!("Loaded {} containers", containers.len());
                Some(containers)
            }
            Err(e) => {
                warn!("Failed to load containers: {}", e);
                None
            }
        };

        let images = match client.list_images(true).await {
            Ok(images) => {
                debug!("Loaded {} images", images.len());
                Some(images)
            }
            Err(e) => {
                warn!("Failed to load images: {}", e);
                None
            }
        };

        let volumes = match client.list_volumes().await {
            Ok(volumes) => {
                debug!("Loaded {} volumes", volumes.len());
                Some(volumes)
            }
            Err(e) => {
                warn!("Failed to load volumes: {}", e);
                None
            }
        };

        let networks = match client.list_networks().await {
            Ok(networks) => {
                debug!("Loaded {} networks", networks.len());
                Some(networks)
            }
            Err(e) => {
                warn!("Failed to load networks: {}", e);
                None
            }
        };

        let disk_usage = match client.get_disk_usage().await {
            Ok(disk_usage) => {
                debug!("Loaded disk usage");
                Some(disk_usage)
            }
            Err(e) => {
                warn!("Failed to load disk usage: {}", e);
                None
            }
        };

        Self {
            containers,
            images,
            volumes,
            networks,
            disk_usage,
        }
    }

    fn apply(self, state: &mut AppState) {
        if let Some(containers) = self.containers {
            state.update_containers(containers);
        }
        if let Some(images) = self.images {
            state.update_images(images);
        }
        if let Some(volumes) = self.volumes {
            state.update_volumes(volumes);
        }
        if let Some(networks) = self.networks {
            state.update_networks(networks);
        }
        if let Some(disk_usage) = self.disk_usage {
            state.update_disk_usage(disk_usage);
        }
    }
}

struct DataRefreshResult {
    client: Option<DockerClient>,
    info: Option<ConnectionInfo>,
    data: DataRefreshData,
}

impl DataRefreshResult {
    async fn gather(client: Option<DockerClient>, config: Config) -> Self {
        if let Some(client) = client {
            debug!("Refreshing data from Docker");
            if let Err(e) = client.ping().await {
                warn!("Lost connection to Docker: {}", e);
                return Self::disconnected();
            }

            let data = DataRefreshData::fetch(&client).await;
            let info = client.connection_info().clone();
            return Self {
                client: Some(client),
                info: Some(info),
                data,
            };
        }

        info!("No Docker client, attempting to connect...");
        match App::connect_docker(&config).await {
            Ok((client, info)) => {
                let data = DataRefreshData::fetch(&client).await;
                Self {
                    client: Some(client),
                    info: Some(info),
                    data,
                }
            }
            Err(e) => {
                warn!("Connection attempt failed: {}", e);
                Self::disconnected()
            }
        }
    }

    fn disconnected() -> Self {
        Self {
            client: None,
            info: None,
            data: DataRefreshData::default(),
        }
    }
}

impl App {
    /// Create a new application instance
    pub async fn new(config: Config) -> Result<Self> {
        info!("Creating new App instance");

        let mut state = AppState::new();

        // Try to connect to Docker
        let docker_client = match Self::connect_docker(&config).await {
            Ok((client, info)) => {
                state.set_docker_connected(true, info);
                Some(client)
            }
            Err(e) => {
                warn!("Could not connect to Docker: {}", e);
                state.set_docker_connected(false, ConnectionInfo::default());
                None
            }
        };

        Ok(Self {
            config,
            state,
            docker_client,
            log_fetch_rx: None,
            last_log_fetch: None,
            stats_fetch_rx: None,
            last_stats_fetch: None,
            data_refresh_handle: None,
            data_refresh_gate: RefreshGate::default(),
            last_data_refresh: std::time::Instant::now(),
            exec_runtime: None,
            exec_start_rx: None,
            exec_start_pending: None,
            last_terminal_size: None,
        })
    }

    /// Connect to Docker
    async fn connect_docker(config: &Config) -> Result<(DockerClient, ConnectionInfo)> {
        let client =
            DockerClient::connect(config.docker.host.as_deref(), config.docker.runtime).await?;

        let info = client.connection_info().clone();
        Ok((client, info))
    }

    /// Run the main application loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting main application loop");

        // Setup terminal
        let mut terminal = setup_terminal()?;

        // Initial data load
        self.refresh_data_once().await;

        // Main event loop
        let result = self.run_event_loop(&mut terminal).await;

        // Cleanup terminal
        restore_terminal(&mut terminal)?;

        result
    }

    /// Run the event loop
    async fn run_event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let mut last_tick = std::time::Instant::now();
        let data_refresh_rate = Duration::from_secs(2); // Refresh data every 2 seconds
        let mut should_quit;

        loop {
            let exec_focused = self
                .state
                .exec_view
                .as_ref()
                .map(|view| view.focus)
                .unwrap_or(false);
            let tick_rate = exec_tick_rate(exec_focused);
            // Render the UI
            let ui_app = UiApp::new(self.state.clone());
            terminal.draw(|f| ui_app.draw(f))?;

            // Handle events with timeout
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout)? {
                let event = crossterm::event::read()?;

                // Create a mutable UI app for handling events
                let mut ui_app = UiApp::new(self.state.clone());
                let action = ui_app.handle_event(event);

                // Apply UI-driven state changes first (navigation, dialogs, log view close/scroll, etc.)
                // and then execute the requested action.
                should_quit = ui_app.should_quit;
                self.state = ui_app.state;
                self.handle_ui_action(action).await;

                // Check if should quit
                if should_quit {
                    info!("Quit signal received, exiting event loop");
                    break;
                }
            }

            // Periodic tasks (every 250ms)
            if last_tick.elapsed() >= tick_rate {
                // Check for completed log fetches
                self.check_log_fetch().await;

                // Check for completed stats fetches
                self.check_stats_fetch().await;

                // Check for completed data refreshes
                self.check_data_refresh().await;

                // Auto-refresh logs when in follow mode (every 2 seconds)
                if let Some(ref log_view) = self.state.log_view {
                    if log_view.follow && self.log_fetch_rx.is_none() {
                        let should_fetch = self
                            .last_log_fetch
                            .map(|t| t.elapsed() >= Duration::from_secs(2))
                            .unwrap_or(true);
                        if should_fetch {
                            let id = log_view.container_id.clone();
                            self.start_log_streaming(id);
                            self.last_log_fetch = Some(std::time::Instant::now());
                        }
                    }
                }

                // Auto-refresh stats when in follow mode (every 1 second)
                // Also switch to follow the selected container if it changed
                let stats_container_id = self
                    .state
                    .stats_view
                    .as_ref()
                    .map(|sv| sv.container_id.clone());
                let selected_id = self.state.selected_container.clone();

                if let (Some(stats_id), Some(selected_id)) = (stats_container_id, selected_id) {
                    if stats_id != selected_id {
                        // Selected container changed - switch stats to new container
                        let name = self
                            .state
                            .containers
                            .iter()
                            .find(|c| c.id == selected_id)
                            .and_then(|c| c.names.first())
                            .cloned()
                            .unwrap_or_else(|| selected_id.chars().take(12).collect::<String>());
                        self.state.open_stats_view(selected_id.clone(), name);
                        self.start_stats_streaming(selected_id);
                        self.last_stats_fetch = Some(std::time::Instant::now());
                    } else if let Some(ref stats_view) = self.state.stats_view {
                        // Same container - just refresh if needed
                        if stats_view.follow && self.stats_fetch_rx.is_none() {
                            let should_fetch = self
                                .last_stats_fetch
                                .map(|t| t.elapsed() >= Duration::from_secs(1))
                                .unwrap_or(true);
                            if should_fetch {
                                self.start_stats_streaming(stats_id);
                                self.last_stats_fetch = Some(std::time::Instant::now());
                            }
                        }
                    }
                }

                // Refresh data periodically (every 2 seconds)
                if self.last_data_refresh.elapsed() >= data_refresh_rate {
                    self.request_data_refresh();
                }

                // Check for exec start completion
                self.check_exec_start().await;
                // Tick exec spinner while starting
                self.tick_exec_spinner();

                // Check for exec output
                self.check_exec_output().await;

                // Resize exec session if terminal size changed
                if self.state.exec_view.is_some() {
                    let current = self.state.terminal_size;
                    if self.last_terminal_size != Some(current) {
                        self.resize_exec_if_needed().await;
                        self.last_terminal_size = Some(current);
                    }
                }
                // Clear old notifications (older than 3 seconds)
                self.state.clear_old_notifications(3);
                last_tick = std::time::Instant::now();
            }
        }

        Ok(())
    }

    /// Handle UI action
    async fn handle_ui_action(&mut self, action: UiAction) {
        match action {
            UiAction::None => {}
            UiAction::Quit => {
                // Handled in event loop
            }
            UiAction::StartContainer(id) => {
                self.start_container(&id).await;
            }
            UiAction::StopContainer(id) => {
                self.stop_container(&id).await;
            }
            UiAction::RestartContainer(id) => {
                self.restart_container(&id).await;
            }
            UiAction::PauseContainer(id) => {
                self.pause_container(&id).await;
            }
            UiAction::UnpauseContainer(id) => {
                self.unpause_container(&id).await;
            }
            UiAction::KillContainer(id) => {
                self.kill_container(&id).await;
            }
            UiAction::RemoveContainer(id) => {
                self.remove_container(&id).await;
            }
            UiAction::ShowContainerLogs(id) => {
                let name = self
                    .state
                    .containers
                    .iter()
                    .find(|c| c.id == id)
                    .and_then(|c| c.names.first())
                    .cloned()
                    .unwrap_or_else(|| id.chars().take(12).collect::<String>());

                let needs_open = self
                    .state
                    .log_view
                    .as_ref()
                    .map(|lv| lv.container_id != id)
                    .unwrap_or(true);

                if needs_open {
                    self.state.open_log_view(id.clone(), name);
                } else if let Some(lv) = &mut self.state.log_view {
                    lv.logs.clear();
                    lv.scroll_offset = 0;
                    lv.follow = true;
                }

                self.state
                    .add_notification("Loading logs...", NotificationLevel::Info);
                self.start_log_streaming(id);
                self.last_log_fetch = Some(std::time::Instant::now());
            }
            UiAction::ShowContainerStats(id) => {
                // Toggle stats panel: close if already showing this container, otherwise open
                let should_close = self
                    .state
                    .stats_view
                    .as_ref()
                    .map(|sv| sv.container_id == id)
                    .unwrap_or(false);

                if should_close {
                    self.state.close_stats_view();
                } else {
                    let name = self
                        .state
                        .containers
                        .iter()
                        .find(|c| c.id == id)
                        .and_then(|c| c.names.first())
                        .cloned()
                        .unwrap_or_else(|| id.chars().take(12).collect::<String>());

                    self.state.open_stats_view(id.clone(), name);
                    self.state
                        .add_notification("Loading stats...", NotificationLevel::Info);
                    self.start_stats_streaming(id);
                    self.last_stats_fetch = Some(std::time::Instant::now());
                }
            }
            UiAction::ShowContainerDetails(id) => {
                let name = self
                    .state
                    .containers
                    .iter()
                    .find(|c| c.id == id)
                    .and_then(|c| c.names.first())
                    .cloned()
                    .unwrap_or_else(|| id.chars().take(12).collect::<String>());

                self.state.open_detail_view(id.clone(), name);
                self.state
                    .add_notification("Loading details...", NotificationLevel::Info);
                self.fetch_container_details(id).await;
            }
            UiAction::ExecContainer(id) => {
                self.start_exec_for_container(&id).await;
            }
            UiAction::StartContainerAndExec(id) => {
                self.start_container(&id).await;
                self.start_exec_for_container(&id).await;
            }
            UiAction::ExecInput(bytes) => {
                self.write_exec_input(bytes).await;
            }
            UiAction::RemoveImage(id) => {
                self.remove_image(&id).await;
            }
            UiAction::PruneImages => {
                self.prune_images().await;
            }
            UiAction::ShowImageDetails(id) => {
                let name = self
                    .state
                    .images
                    .iter()
                    .find(|i| i.id == id)
                    .and_then(|i| i.repo_tags.first())
                    .cloned()
                    .unwrap_or_else(|| id.chars().take(12).collect::<String>());

                self.state.open_image_detail_view(id.clone(), name);
                self.state
                    .add_notification("Loading image details...", NotificationLevel::Info);
                self.fetch_image_details(id).await;
            }
            UiAction::RemoveVolume(name) => {
                self.remove_volume(&name).await;
            }
            UiAction::PruneVolumes => {
                self.prune_volumes().await;
            }
            UiAction::RemoveNetwork(id) => {
                self.remove_network(&id).await;
            }
            UiAction::PruneNetworks => {
                self.prune_networks().await;
            }
            UiAction::ExportLogs => {
                self.export_logs();
            }
            UiAction::Clear => {
                // No-op: terminal clear is handled by the render cycle
            }
            UiAction::PruneSystem => {
                self.prune_system().await;
            }
        }
    }

    /// Start a container
    async fn start_container(&mut self, id: &str) {
        if let Some(client) = &self.docker_client {
            info!("Starting container {}", id);
            match client.start_container(id).await {
                Ok(_) => {
                    info!("Container {} started", id);
                    self.state
                        .add_notification("Container started", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to start container {}: {}", id, e);
                    self.state.add_notification(
                        format!("Failed to start: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        } else {
            self.state
                .add_notification("Docker not connected", NotificationLevel::Error);
        }
    }

    /// Stop a container
    async fn stop_container(&mut self, id: &str) {
        if let Some(client) = &self.docker_client {
            info!("Stopping container {}", id);
            match client.stop_container(id, Some(10)).await {
                Ok(_) => {
                    info!("Container {} stopped", id);
                    self.state
                        .add_notification("Container stopped", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to stop container {}: {}", id, e);
                    self.state.add_notification(
                        format!("Failed to stop: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        } else {
            self.state
                .add_notification("Docker not connected", NotificationLevel::Error);
        }
    }

    /// Restart a container
    async fn restart_container(&mut self, id: &str) {
        if let Some(client) = &self.docker_client {
            info!("Restarting container {}", id);
            match client.restart_container(id, Some(10)).await {
                Ok(_) => {
                    info!("Container {} restarted", id);
                    self.state
                        .add_notification("Container restarted", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to restart container {}: {}", id, e);
                    self.state.add_notification(
                        format!("Failed to restart: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        } else {
            self.state
                .add_notification("Docker not connected", NotificationLevel::Error);
        }
    }

    /// Pause a container
    async fn pause_container(&mut self, id: &str) {
        if let Some(client) = &self.docker_client {
            info!("Pausing container {}", id);
            match client.pause_container(id).await {
                Ok(_) => {
                    info!("Container {} paused", id);
                    self.state
                        .add_notification("Container paused", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to pause container {}: {}", id, e);
                    self.state.add_notification(
                        format!("Failed to pause: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        } else {
            self.state
                .add_notification("Docker not connected", NotificationLevel::Error);
        }
    }

    /// Unpause a container
    async fn unpause_container(&mut self, id: &str) {
        if let Some(client) = &self.docker_client {
            info!("Unpausing container {}", id);
            match client.unpause_container(id).await {
                Ok(_) => {
                    info!("Container {} unpaused", id);
                    self.state
                        .add_notification("Container unpaused", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to unpause container {}: {}", id, e);
                    self.state.add_notification(
                        format!("Failed to unpause: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        } else {
            self.state
                .add_notification("Docker not connected", NotificationLevel::Error);
        }
    }

    /// Kill a container
    async fn kill_container(&mut self, id: &str) {
        if let Some(client) = &self.docker_client {
            info!("Killing container {}", id);
            match client.kill_container(id, None).await {
                Ok(_) => {
                    info!("Container {} killed", id);
                    self.state
                        .add_notification("Container killed", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to kill container {}: {}", id, e);
                    self.state.add_notification(
                        format!("Failed to kill: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        } else {
            self.state
                .add_notification("Docker not connected", NotificationLevel::Error);
        }
    }

    /// Remove a container
    async fn remove_container(&mut self, id: &str) {
        if let Some(client) = &self.docker_client {
            info!("Removing container {}", id);
            match client.remove_container(id, false, false).await {
                Ok(_) => {
                    info!("Container {} removed", id);
                    self.state
                        .add_notification("Container removed", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to remove container {}: {}", id, e);
                    self.state.add_notification(
                        format!("Failed to remove: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        } else {
            self.state
                .add_notification("Docker not connected", NotificationLevel::Error);
        }
    }

    /// Refresh all data from Docker (blocking, used for initial load)
    async fn refresh_data_once(&mut self) {
        let had_client = self.docker_client.is_some();
        let result =
            DataRefreshResult::gather(self.docker_client.clone(), self.config.clone()).await;
        self.apply_refresh_result(result, had_client);
        self.last_data_refresh = std::time::Instant::now();
    }

    /// Request a background data refresh
    fn request_data_refresh(&mut self) {
        self.last_data_refresh = std::time::Instant::now();
        if !self.data_refresh_gate.request() {
            return;
        }

        let (tx, rx) = mpsc::channel(1);
        let had_client = self.docker_client.is_some();
        self.data_refresh_handle = Some(DataRefreshHandle { rx, had_client });

        let client = self.docker_client.clone();
        let config = self.config.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create runtime");

            rt.block_on(async move {
                let result = DataRefreshResult::gather(client, config).await;
                let _ = tx.send(result).await;
            });
        });
    }

    /// Check for completed data refreshes
    async fn check_data_refresh(&mut self) {
        let mut received: Option<(bool, Option<DataRefreshResult>)> = None;
        if let Some(handle) = &mut self.data_refresh_handle {
            match tokio::time::timeout(Duration::from_millis(1), handle.rx.recv()).await {
                Ok(Some(result)) => {
                    received = Some((handle.had_client, Some(result)));
                }
                Ok(None) => {
                    received = Some((handle.had_client, None));
                }
                Err(_) => {}
            }
        }

        if let Some((had_client, result)) = received {
            self.data_refresh_handle = None;
            if let Some(result) = result {
                self.apply_refresh_result(result, had_client);
            }
            if self.data_refresh_gate.complete() {
                self.request_data_refresh();
            }
        }
    }

    fn apply_refresh_result(&mut self, result: DataRefreshResult, had_client: bool) {
        match result.client {
            Some(client) => {
                let info = result
                    .info
                    .unwrap_or_else(|| client.connection_info().clone());
                if !self.state.docker_connected {
                    let message = if had_client {
                        "Docker connection restored"
                    } else {
                        "Connected to Docker"
                    };
                    self.state
                        .add_notification(message, NotificationLevel::Success);
                }
                self.state.set_docker_connected(true, info);
                self.docker_client = Some(client);
                result.data.apply(&mut self.state);
            }
            None => {
                if self.state.docker_connected {
                    self.state.docker_connected = false;
                    self.state
                        .add_notification("Lost connection to Docker", NotificationLevel::Error);
                }
                self.docker_client = None;
            }
        }
    }

    /// Remove an image
    async fn remove_image(&mut self, id: &str) {
        if let Some(client) = &self.docker_client {
            info!("Removing image {}", id);
            match client.remove_image(id, false).await {
                Ok(_) => {
                    info!("Image {} removed", id);
                    self.state
                        .add_notification("Image removed", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to remove image {}: {}", id, e);
                    self.state.add_notification(
                        format!("Failed to remove image: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        }
    }

    /// Prune dangling images
    async fn prune_images(&mut self) {
        if let Some(client) = &self.docker_client {
            info!("Pruning dangling images");
            match client.prune_images().await {
                Ok(reclaimed) => {
                    let size_str = format_size(reclaimed);
                    info!("Pruned images, reclaimed {}", size_str);
                    self.state.add_notification(
                        format!("Pruned images, reclaimed {}", size_str),
                        NotificationLevel::Success,
                    );
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to prune images: {}", e);
                    self.state.add_notification(
                        format!("Failed to prune images: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        }
    }

    /// Remove a volume
    async fn remove_volume(&mut self, name: &str) {
        if let Some(client) = &self.docker_client {
            info!("Removing volume {}", name);
            match client.remove_volume(name, false).await {
                Ok(_) => {
                    info!("Volume {} removed", name);
                    self.state
                        .add_notification("Volume removed", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to remove volume {}: {}", name, e);
                    self.state.add_notification(
                        format!("Failed to remove volume: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        }
    }

    /// Prune unused volumes
    async fn prune_volumes(&mut self) {
        if let Some(client) = &self.docker_client {
            info!("Pruning volumes");
            match client.prune_volumes().await {
                Ok(reclaimed) => {
                    let size_str = format_size(reclaimed);
                    info!("Pruned volumes, reclaimed {}", size_str);
                    self.state.add_notification(
                        format!("Pruned volumes, reclaimed {}", size_str),
                        NotificationLevel::Success,
                    );
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to prune volumes: {}", e);
                    self.state.add_notification(
                        format!("Failed to prune volumes: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        }
    }

    /// Remove a network
    async fn remove_network(&mut self, id: &str) {
        if let Some(client) = &self.docker_client {
            info!("Removing network {}", id);
            match client.remove_network(id).await {
                Ok(_) => {
                    info!("Network {} removed", id);
                    self.state
                        .add_notification("Network removed", NotificationLevel::Success);
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to remove network {}: {}", id, e);
                    self.state.add_notification(
                        format!("Failed to remove network: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        }
    }

    /// Prune unused networks
    async fn prune_networks(&mut self) {
        if let Some(client) = &self.docker_client {
            info!("Pruning networks");
            match client.prune_networks().await {
                Ok(count) => {
                    info!("Pruned {} networks", count);
                    self.state.add_notification(
                        format!("Pruned {} networks", count),
                        NotificationLevel::Success,
                    );
                    self.request_data_refresh();
                }
                Err(e) => {
                    error!("Failed to prune networks: {}", e);
                    self.state.add_notification(
                        format!("Failed to prune networks: {}", e),
                        NotificationLevel::Error,
                    );
                }
            }
        }
    }

    /// Prune system resources based on selected options
    async fn prune_system(&mut self) {
        // Get options first, then close dialog
        let options = self.state.get_prune_options();
        self.state.close_prune_dialog();

        if let Some(client) = &self.docker_client {
            if let Some(options) = options {
                info!(
                    "Pruning system resources: containers={}, images={}, volumes={}, networks={}",
                    options.containers, options.images, options.volumes, options.networks
                );

                let mut total_reclaimed: i64 = 0;
                let mut has_error = false;
                let mut pruned_containers = 0usize;
                let mut pruned_images = 0u64;
                let mut pruned_volumes = 0u64;
                let mut pruned_networks = 0u64;

                // Prune containers
                if options.containers {
                    match client.prune_containers_detailed().await {
                        Ok(result) => {
                            total_reclaimed += result.space_reclaimed;
                            pruned_containers = result.items_deleted.len();
                            info!("Pruned {} containers", pruned_containers);
                        }
                        Err(e) => {
                            error!("Failed to prune containers: {}", e);
                            has_error = true;
                        }
                    }
                }

                // Prune images
                if options.images {
                    match client.prune_images().await {
                        Ok(reclaimed) => {
                            total_reclaimed += reclaimed as i64;
                            pruned_images = reclaimed;
                            info!("Pruned images, reclaimed {} bytes", reclaimed);
                        }
                        Err(e) => {
                            error!("Failed to prune images: {}", e);
                            has_error = true;
                        }
                    }
                }

                // Prune volumes
                if options.volumes {
                    info!("Pruning volumes...");
                    match client.prune_volumes().await {
                        Ok(reclaimed) => {
                            total_reclaimed += reclaimed as i64;
                            pruned_volumes = reclaimed;
                            info!("Pruned volumes, reclaimed {} bytes", reclaimed);
                            if reclaimed == 0 {
                                info!("No unused volumes found to prune - volumes may still be referenced by stopped containers");
                            }
                        }
                        Err(e) => {
                            error!("Failed to prune volumes: {}", e);
                            has_error = true;
                        }
                    }
                }

                // Prune networks
                if options.networks {
                    match client.prune_networks().await {
                        Ok(count) => {
                            pruned_networks = count;
                            info!("Pruned {} networks", count);
                        }
                        Err(e) => {
                            error!("Failed to prune networks: {}", e);
                            has_error = true;
                        }
                    }
                }

                // Note: Build cache pruning removed - not available in bollard 0.18

                // Build detailed notification message
                let size_str = crate::docker::format_bytes_size(total_reclaimed);
                let mut details = vec![];
                if options.containers && pruned_containers > 0 {
                    details.push(format!("{} containers", pruned_containers));
                }
                if options.images && pruned_images > 0 {
                    details.push("images".to_string());
                }
                if options.volumes && pruned_volumes > 0 {
                    details.push("volumes".to_string());
                }
                if options.networks && pruned_networks > 0 {
                    details.push(format!("{} networks", pruned_networks));
                }

                let message = if details.is_empty() {
                    format!("Nothing to prune (reclaimable: {})", size_str)
                } else {
                    format!("Pruned {} ({})", size_str, details.join(", "))
                };

                let level = if has_error {
                    NotificationLevel::Warning
                } else if details.is_empty() {
                    NotificationLevel::Info
                } else {
                    NotificationLevel::Success
                };

                self.state.add_notification(message, level);

                // Refresh data to show updated disk usage
                self.request_data_refresh();
            }
        }
    }

    /// Start fetching logs from a container (non-blocking, uses channel)
    fn start_log_streaming(&mut self, container_id: String) {
        if let Some(client) = &self.docker_client {
            // Cancel any existing pending fetch by dropping the receiver
            if self.log_fetch_rx.is_some() {
                info!("Cancelling previous log fetch");
                self.log_fetch_rx = None;
            }

            info!("Starting log fetch for container '{}'", container_id);
            self.state
                .add_notification("Fetching logs...", NotificationLevel::Info);

            // Create channel for results
            let (tx, rx) = mpsc::channel(1);
            self.log_fetch_rx = Some(rx);

            // Clone client for the thread
            let client = client.clone();

            // Spawn a completely separate OS thread with its own runtime
            // This is the only way to ensure bollard's blocking operations don't freeze our UI
            std::thread::spawn(move || {
                // Create a new runtime just for this thread
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create runtime");

                rt.block_on(async move {
                    // fetch_logs has per-item timeouts
                    let result = client.fetch_logs(&container_id, 100).await;

                    // Send result back via channel (ignore send errors if receiver dropped)
                    let _ = tx.send(result).await;
                });
            });
        } else {
            self.state
                .add_notification("Not connected to Docker", NotificationLevel::Error);
        }
    }

    /// Check if log fetch has completed and update state
    async fn check_log_fetch(&mut self) {
        if let Some(rx) = &mut self.log_fetch_rx {
            // Try to receive with a very short timeout (non-blocking check)
            match tokio::time::timeout(Duration::from_millis(1), rx.recv()).await {
                Ok(Some(Ok(entries))) => {
                    self.log_fetch_rx = None; // Clear the receiver
                    let count = entries.len();
                    info!("Fetched {} log entries", count);
                    if count > 0 {
                        for entry in entries {
                            self.state.add_log_entry(entry);
                        }
                        // Don't show notification for auto-refreshes in follow mode
                        // (only manual 'r' press shows notification)
                    }
                }
                Ok(Some(Err(e))) => {
                    self.log_fetch_rx = None;
                    warn!("Failed to fetch logs: {}", e);
                    self.state.add_notification(
                        format!("Failed to fetch logs: {}", e),
                        NotificationLevel::Error,
                    );
                }
                Ok(None) => {
                    // Channel closed
                    self.log_fetch_rx = None;
                }
                Err(_) => {
                    // Timeout - still waiting, that's fine
                }
            }
        }
    }

    /// Fetch container details
    async fn fetch_container_details(&mut self, container_id: String) {
        if let Some(client) = &self.docker_client {
            info!("Fetching details for container '{}'", container_id);

            match client.inspect_container(&container_id).await {
                Ok(details) => {
                    info!("Fetched details for container '{}'", container_id);
                    self.state.set_detail_view_content(details);
                }
                Err(e) => {
                    error!(
                        "Failed to fetch details for container '{}': {}",
                        container_id, e
                    );
                    self.state.add_notification(
                        format!("Failed to fetch details: {}", e),
                        NotificationLevel::Error,
                    );
                    self.state.close_detail_view();
                }
            }
        } else {
            self.state
                .add_notification("Not connected to Docker", NotificationLevel::Error);
            self.state.close_detail_view();
        }
    }

    /// Fetch image details
    async fn fetch_image_details(&mut self, image_id: String) {
        if let Some(client) = &self.docker_client {
            info!("Fetching details for image '{}'", image_id);

            match client.inspect_image(&image_id).await {
                Ok(details) => {
                    info!("Fetched details for image '{}'", image_id);
                    self.state.set_image_detail_view_content(details);
                }
                Err(e) => {
                    error!("Failed to fetch details for image '{}': {}", image_id, e);
                    self.state.add_notification(
                        format!("Failed to fetch image details: {}", e),
                        NotificationLevel::Error,
                    );
                    self.state.close_image_detail_view();
                }
            }
        } else {
            self.state
                .add_notification("Not connected to Docker", NotificationLevel::Error);
            self.state.close_image_detail_view();
        }
    }

    /// Export currently visible logs to file
    fn export_logs(&mut self) {
        use crate::state::LogLevelFilter;

        if let Some(ref log_view) = self.state.log_view {
            // Generate default filename
            let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
            let default_name = format!(
                "{}_{}.log",
                log_view.container_name.replace('/', "_"),
                timestamp
            );

            // For now, use a simple approach - save to current directory with default name
            let filename = &default_name;

            // Helper function to detect log level from message
            fn detect_log_level(message: &str) -> crate::state::LogLevelFilter {
                let upper = message.to_uppercase();
                if upper.contains("ERROR") || upper.contains("FATAL") {
                    LogLevelFilter::Error
                } else if upper.contains("WARN") {
                    LogLevelFilter::Warn
                } else if upper.contains("INFO") {
                    LogLevelFilter::Info
                } else {
                    LogLevelFilter::All // Default
                }
            }

            // Compile search regex if pattern exists
            let search_regex = log_view
                .search_pattern
                .as_ref()
                .and_then(|p| regex::Regex::new(p).ok());

            // Build log content
            let mut content = String::new();
            content.push_str(&format!(
                "# Logs for container: {}\n",
                log_view.container_name
            ));
            content.push_str(&format!(
                "# Exported: {}\n",
                chrono::Local::now().to_rfc3339()
            ));
            if let Some(ref pattern) = log_view.search_pattern {
                content.push_str(&format!("# Search pattern: {}\n", pattern));
            }
            if log_view.level_filter != LogLevelFilter::All {
                content.push_str(&format!("# Level filter: {:?}\n", log_view.level_filter));
            }
            if log_view.time_filter.is_some() {
                content.push_str(&format!(
                    "# Time filter: logs after {}\n",
                    log_view
                        .time_filter
                        .unwrap()
                        .format("%Y-%m-%d %H:%M:%S UTC")
                ));
            }
            content.push_str("#\n");

            // Filter and add logs
            let mut exported_count = 0;
            for entry in &log_view.logs {
                // Apply level filter
                let level_match = match log_view.level_filter {
                    LogLevelFilter::All => true,
                    _ => detect_log_level(&entry.message) == log_view.level_filter,
                };

                // Apply time filter
                let time_match = log_view.time_filter.map_or(true, |cutoff| {
                    entry.timestamp.is_some_and(|ts| ts >= cutoff)
                });

                // Apply search filter
                let search_match = log_view.search_pattern.as_ref().map_or(true, |pattern| {
                    if let Some(ref re) = search_regex {
                        re.is_match(&entry.message)
                    } else {
                        entry
                            .message
                            .to_lowercase()
                            .contains(&pattern.to_lowercase())
                    }
                });

                if level_match && time_match && search_match {
                    let timestamp = entry
                        .timestamp
                        .map(|ts| ts.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "??:??:??".to_string());
                    content.push_str(&format!("[{}] {}\n", timestamp, entry.message));
                    exported_count += 1;
                }
            }

            // Write to file
            match std::fs::write(filename, content) {
                Ok(_) => {
                    let msg = format!("Exported {} logs to {}", exported_count, filename);
                    info!("{}", msg);
                    self.state.add_notification(msg, NotificationLevel::Success);
                }
                Err(e) => {
                    let msg = format!("Failed to export logs: {}", e);
                    error!("{}", msg);
                    self.state.add_notification(msg, NotificationLevel::Error);
                }
            }
        }
    }

    /// Start fetching stats from a container (non-blocking, uses channel)
    fn start_stats_streaming(&mut self, container_id: String) {
        if let Some(client) = &self.docker_client {
            // Cancel any existing pending fetch by dropping the receiver
            if self.stats_fetch_rx.is_some() {
                info!("Cancelling previous stats fetch");
                self.stats_fetch_rx = None;
            }

            info!("Starting stats fetch for container '{}'", container_id);

            // Create channel for results
            let (tx, rx) = mpsc::channel(1);
            self.stats_fetch_rx = Some(rx);

            // Clone client for the thread
            let client = client.clone();

            // Spawn a separate OS thread with its own runtime
            std::thread::spawn(move || {
                // Create a new runtime just for this thread
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create runtime");

                rt.block_on(async move {
                    // fetch_stats has timeout built-in
                    let result = client.fetch_stats(&container_id).await;

                    // Send result back via channel (ignore send errors if receiver dropped)
                    let _ = tx.send(result).await;
                });
            });
        } else {
            self.state
                .add_notification("Not connected to Docker", NotificationLevel::Error);
        }
    }

    /// Check if stats fetch has completed and update state
    async fn check_stats_fetch(&mut self) {
        if let Some(rx) = &mut self.stats_fetch_rx {
            // Try to receive with a very short timeout (non-blocking check)
            match tokio::time::timeout(Duration::from_millis(1), rx.recv()).await {
                Ok(Some(Ok(stats))) => {
                    self.stats_fetch_rx = None; // Clear the receiver
                    info!("Fetched stats");
                    self.state.update_stats(stats);
                }
                Ok(Some(Err(e))) => {
                    self.stats_fetch_rx = None;
                    warn!("Failed to fetch stats: {}", e);
                    self.state.set_stats_error(format!("{}", e));
                    self.state.add_notification(
                        format!("Failed to fetch stats: {}", e),
                        NotificationLevel::Error,
                    );
                }
                Ok(None) => {
                    // Channel closed
                    self.stats_fetch_rx = None;
                }
                Err(_) => {
                    // Timeout - still waiting, that's fine
                }
            }
        }
    }

    // ==================== Exec Handling ====================

    async fn start_exec_for_container(&mut self, id: &str) {
        if let Some(runtime) = &self.exec_runtime {
            if runtime.container_id == id {
                self.state.close_exec_view();
                self.exec_runtime = None;
                return;
            }
            self.state.add_notification(
                "Exec already active for another container",
                NotificationLevel::Warning,
            );
            return;
        }

        if let Some(pending) = &self.exec_start_pending {
            if pending.container_id == id {
                self.state.close_exec_view();
                self.exec_start_pending = None;
                self.exec_start_rx = None;
                return;
            }
            self.state.add_notification(
                "Exec already starting for another container",
                NotificationLevel::Warning,
            );
            return;
        }

        let client = match &self.docker_client {
            Some(client) => client.clone(),
            None => {
                self.state
                    .add_notification("Docker not connected", NotificationLevel::Error);
                return;
            }
        };

        let container_name = self
            .state
            .containers
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.names.first())
            .cloned()
            .unwrap_or_else(|| id.chars().take(12).collect::<String>());
        let (cols, rows) =
            compute_exec_pane_size(self.state.terminal_size.0, self.state.terminal_size.1);

        self.state
            .open_exec_view(id.to_string(), container_name.clone());
        self.state
            .set_exec_status(format_exec_start_status(spinner::frame(0)));

        self.exec_start_pending = Some(ExecStartPending {
            container_id: id.to_string(),
            spinner_index: 0,
        });

        let (tx, rx) = mpsc::channel(1);
        self.exec_start_rx = Some(rx);

        let container_id = id.to_string();
        tokio::spawn(async move {
            let defaults = match client.exec_defaults(&container_id).await {
                Ok(d) => d,
                Err(e) => {
                    let _ = tx
                        .send(ExecStartResult::Failed {
                            container_id,
                            message: format!("Failed to inspect container: {e}"),
                        })
                        .await;
                    return;
                }
            };

            if !defaults.running {
                let _ = tx
                    .send(ExecStartResult::Failed {
                        container_id: defaults.container_id,
                        message: "Container is not running".to_string(),
                    })
                    .await;
                return;
            }

            let cmd = select_exec_command(&defaults.entrypoint, &defaults.cmd);
            let exec = match client
                .start_exec_session(&defaults.container_id, cmd, cols, rows)
                .await
            {
                Ok(exec) => exec,
                Err(e) => {
                    let _ = tx
                        .send(ExecStartResult::Failed {
                            container_id: defaults.container_id,
                            message: format!("Failed to start exec: {e}"),
                        })
                        .await;
                    return;
                }
            };

            let container_name = if defaults.container_name.is_empty() {
                container_name
            } else {
                defaults.container_name
            };

            let _ = tx
                .send(ExecStartResult::Started {
                    exec,
                    container_id: defaults.container_id,
                    container_name,
                    size: (cols, rows),
                })
                .await;
        });
    }

    async fn write_exec_input(&mut self, bytes: Vec<u8>) {
        if let Some(runtime) = &mut self.exec_runtime {
            if let Err(e) = runtime.input.write_all(&bytes).await {
                self.state
                    .add_notification(format!("Exec input failed: {e}"), NotificationLevel::Error);
            }
        }
    }

    async fn check_exec_start(&mut self) {
        if let Some(rx) = &mut self.exec_start_rx {
            match tokio::time::timeout(Duration::from_millis(1), rx.recv()).await {
                Ok(Some(result)) => {
                    self.exec_start_rx = None;
                    match result {
                        ExecStartResult::Started {
                            exec,
                            container_id,
                            container_name,
                            size,
                        } => {
                            let matches_pending = self
                                .exec_start_pending
                                .as_ref()
                                .map(|p| p.container_id == container_id)
                                .unwrap_or(false);
                            if !matches_pending {
                                return;
                            }
                            self.exec_start_pending = None;

                            let (tx, rx) = mpsc::channel(64);
                            let mut output = exec.output;
                            tokio::spawn(async move {
                                while let Some(item) = output.next().await {
                                    match item {
                                        Ok(log) => {
                                            let bytes = match log {
                                                bollard::container::LogOutput::StdOut {
                                                    message,
                                                } => message.to_vec(),
                                                bollard::container::LogOutput::StdErr {
                                                    message,
                                                } => message.to_vec(),
                                                bollard::container::LogOutput::Console {
                                                    message,
                                                } => message.to_vec(),
                                                bollard::container::LogOutput::StdIn {
                                                    message,
                                                } => message.to_vec(),
                                            };
                                            if tx.send(ExecOutput::Bytes(bytes)).await.is_err() {
                                                break;
                                            }
                                        }
                                        Err(_) => {
                                            let _ = tx.send(ExecOutput::End).await;
                                            return;
                                        }
                                    }
                                }
                                let _ = tx.send(ExecOutput::End).await;
                            });

                            let parser = vt100::Parser::new(size.1, size.0, 0);
                            if let Some(exec_view) = &mut self.state.exec_view {
                                exec_view.container_id = container_id.clone();
                                exec_view.container_name = container_name;
                            }
                            self.state.set_exec_status("Running");
                            self.state
                                .add_notification("Exec started", NotificationLevel::Info);

                            self.exec_runtime = Some(ExecRuntime {
                                exec_id: exec.exec_id,
                                container_id,
                                input: exec.input,
                                parser,
                                output_rx: rx,
                                size,
                            });
                        }
                        ExecStartResult::Failed {
                            container_id,
                            message,
                        } => {
                            let matches_pending = self
                                .exec_start_pending
                                .as_ref()
                                .map(|p| p.container_id == container_id)
                                .unwrap_or(false);
                            if !matches_pending {
                                return;
                            }
                            self.exec_start_pending = None;
                            self.state.set_exec_status(format!("Failed: {}", message));
                            self.state.add_notification(
                                format!("Exec failed: {}", message),
                                NotificationLevel::Error,
                            );
                        }
                    }
                }
                Ok(None) => {
                    self.exec_start_rx = None;
                    if self.exec_start_pending.is_some() {
                        self.exec_start_pending = None;
                        self.state
                            .set_exec_status("Failed: exec start canceled".to_string());
                        self.state
                            .add_notification("Exec start canceled", NotificationLevel::Error);
                    }
                }
                Err(_) => {}
            }
        }
    }

    fn tick_exec_spinner(&mut self) {
        if let Some(pending) = &mut self.exec_start_pending {
            let frame = spinner::frame(pending.spinner_index);
            self.state.set_exec_status(format_exec_start_status(frame));
            pending.spinner_index = spinner::next_index(pending.spinner_index);
        }
    }

    async fn check_exec_output(&mut self) {
        let mut lines: Option<Vec<String>> = None;
        let mut cursor: Option<Option<(u16, u16)>> = None;
        let mut end = false;

        if let Some(runtime) = &mut self.exec_runtime {
            while let Ok(msg) = runtime.output_rx.try_recv() {
                match msg {
                    ExecOutput::Bytes(bytes) => {
                        runtime.parser.process(&bytes);
                        let screen = runtime.parser.screen();
                        let contents = screen.contents();
                        lines = Some(contents.lines().map(|l| l.to_string()).collect());
                        cursor = Some(if screen.hide_cursor() {
                            None
                        } else {
                            Some(screen.cursor_position())
                        });
                    }
                    ExecOutput::End => {
                        end = true;
                        break;
                    }
                }
            }
        }

        if let Some(lines) = lines {
            self.state.update_exec_screen(lines, None);
        }
        if let Some(cursor) = cursor {
            self.state.set_exec_cursor(cursor);
        }

        if end {
            self.state.close_exec_view();
            self.exec_runtime = None;
            self.state
                .add_notification("Exec ended", NotificationLevel::Info);
        }
    }

    async fn resize_exec_if_needed(&mut self) {
        let (cols, rows) =
            compute_exec_pane_size(self.state.terminal_size.0, self.state.terminal_size.1);

        if let Some(runtime) = &mut self.exec_runtime {
            if runtime.size != (cols, rows) {
                runtime.size = (cols, rows);
                runtime.parser.set_size(rows, cols);
                if let Some(client) = &self.docker_client {
                    let _ = client
                        .resize_exec_session(&runtime.exec_id, cols, rows)
                        .await;
                }
            }
        }
    }
}

/// Format size in human readable format
fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if size == 0 {
        return "0 B".to_string();
    }
    let size = size as f64;
    let exp = (size.ln() / 1024_f64.ln()).min(UNITS.len() as f64 - 1.0) as usize;
    let size = size / 1024_f64.powi(exp as i32);
    format!("{:.1} {}", size, UNITS[exp])
}

/// Setup the terminal for TUI
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    info!("Setting up terminal");

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    info!("Terminal setup complete");
    Ok(terminal)
}

/// Restore terminal to original state
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    info!("Restoring terminal");

    terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    info!("Terminal restored");
    Ok(())
}

fn compute_exec_pane_size(term_width: u16, term_height: u16) -> (u16, u16) {
    let sidebar_width = (term_width / 5).clamp(12, 20);
    let content_width = term_width.saturating_sub(sidebar_width);
    let list_width = (content_width * 60) / 100;

    let cols = list_width.saturating_sub(2).max(1);
    let rows = crate::ui::components::exec_viewer::EXEC_PANEL_HEIGHT
        .saturating_sub(2)
        .min(term_height.saturating_sub(2))
        .max(1);

    (cols, rows)
}

fn format_exec_start_status(frame: &str) -> String {
    format!("Starting {}", frame)
}

fn exec_tick_rate(exec_focused: bool) -> Duration {
    if exec_focused {
        Duration::from_millis(50)
    } else {
        Duration::from_millis(250)
    }
}

#[cfg(test)]
mod tests {
    // Note: Most tests would require async runtime and Docker
    use super::{compute_exec_pane_size, exec_tick_rate, format_exec_start_status, RefreshGate};
    use std::time::Duration;

    #[test]
    fn computes_exec_pane_size() {
        let (cols, rows) = compute_exec_pane_size(120, 30);
        assert!(cols > 0);
        assert!(rows > 0);
    }

    #[test]
    fn formats_exec_start_status_with_spinner() {
        assert_eq!(format_exec_start_status("|"), "Starting |");
    }

    #[test]
    fn exec_tick_rate_changes_with_focus() {
        assert_eq!(exec_tick_rate(true), Duration::from_millis(50));
        assert_eq!(exec_tick_rate(false), Duration::from_millis(250));
    }

    #[test]
    fn refresh_gate_starts_when_idle() {
        let mut gate = RefreshGate::default();

        assert!(gate.request());
        assert!(gate.in_flight);
        assert!(!gate.pending);
    }

    #[test]
    fn refresh_gate_queues_when_in_flight() {
        let mut gate = RefreshGate::default();

        assert!(gate.request());
        assert!(!gate.request());
        assert!(gate.in_flight);
        assert!(gate.pending);
    }

    #[test]
    fn refresh_gate_completes_without_pending() {
        let mut gate = RefreshGate::default();

        assert!(gate.request());
        assert!(!gate.complete());
        assert!(!gate.in_flight);
        assert!(!gate.pending);
    }

    #[test]
    fn refresh_gate_completes_with_pending() {
        let mut gate = RefreshGate::default();

        assert!(gate.request());
        assert!(!gate.request());
        assert!(gate.complete());
        assert!(!gate.in_flight);
        assert!(!gate.pending);
    }
}

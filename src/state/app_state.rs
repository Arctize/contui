//! Application state management

use chrono::Utc;

use crate::core::{
    ConfirmAction, ConnectionInfo, ContainerSummary, ImageSummary, NetworkSummary,
    NotificationLevel, Tab, VolumeSummary,
};
use crate::docker::{PruneOptions, SystemDiskUsage};

/// Main application state
#[derive(Debug, Clone)]
pub struct AppState {
    // Navigation
    pub current_tab: Tab,
    pub previous_tab: Option<Tab>,
    pub focused_panel: Panel,

    // Docker data
    pub containers: Vec<ContainerSummary>,
    pub selected_container: Option<String>,
    pub container_list_selected: usize,
    pub images: Vec<ImageSummary>,
    pub selected_image: Option<String>,
    pub image_list_selected: usize,
    pub volumes: Vec<VolumeSummary>,
    pub selected_volume: Option<String>,
    pub volume_list_selected: usize,
    pub networks: Vec<NetworkSummary>,
    pub selected_network: Option<String>,
    pub network_list_selected: usize,

    // Connection
    pub docker_connected: bool,
    pub connection_info: ConnectionInfo,

    // System disk usage
    pub disk_usage: Option<SystemDiskUsage>,

    // UI state
    pub terminal_size: (u16, u16),
    pub show_help: bool,
    pub notifications: Vec<Notification>,
    pub confirm_dialog: Option<ConfirmAction>,

    // Log view state
    pub log_view: Option<LogViewState>,

    // Stats view state
    pub stats_view: Option<StatsViewState>,

    // Exec view state
    pub exec_view: Option<ExecViewState>,

    // Detail view state
    pub detail_view: Option<DetailViewState>,

    // Image detail view state
    pub image_detail_view: Option<ImageDetailViewState>,

    // Prune dialog state
    pub prune_dialog: Option<PruneDialogState>,

    // Async operations tracking
    pub loading: bool,
}

/// Log level filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevelFilter {
    All,
    Error,
    Warn,
    Info,
}

/// Log view state
#[derive(Debug, Clone)]
pub struct LogViewState {
    pub container_id: String,
    pub container_name: String,
    pub logs: Vec<crate::docker::LogEntry>,
    pub scroll_offset: usize,
    pub follow: bool,
    pub max_lines: usize,
    // Search functionality
    pub search_pattern: Option<String>,
    pub search_matches: Vec<usize>, // indices of matching log entries
    pub current_match: Option<usize>, // index into search_matches
    pub show_search_input: bool,
    // Filter functionality
    pub level_filter: LogLevelFilter,
    pub time_filter: Option<chrono::DateTime<chrono::Utc>>, // show logs after this time
    pub show_time_input: bool,
}

/// Stats view state
#[derive(Debug, Clone)]
pub struct StatsViewState {
    pub container_id: String,
    pub container_name: String,
    pub stats: Option<crate::docker::StatsEntry>,
    pub follow: bool,
    pub error: Option<String>,
}

/// Exec view state
#[derive(Debug, Clone)]
pub struct ExecViewState {
    pub container_id: String,
    pub container_name: String,
    pub focus: bool,
    pub status: String,
    pub screen_lines: Vec<String>,
    pub cursor: Option<(u16, u16)>,
    /// The session is an attach to the main process, not an exec
    pub attach: bool,
}

/// Search state shared by the detail viewers
#[derive(Debug, Clone, Default)]
pub struct ViewSearch {
    /// The query is being typed
    pub input: bool,
    pub query: String,
    /// Ordinal of the current match (taken modulo the number of matches)
    pub current: usize,
}

/// Detail view state
#[derive(Debug, Clone)]
pub struct DetailViewState {
    pub container_id: String,
    pub container_name: String,
    pub details: Option<crate::docker::ContainerDetails>,
    pub scroll_offset: usize,
    pub search: ViewSearch,
}

/// Image detail view state
#[derive(Debug, Clone)]
pub struct ImageDetailViewState {
    pub image_id: String,
    pub image_name: String,
    pub details: Option<crate::docker::ImageDetails>,
    pub scroll_offset: usize,
    pub search: ViewSearch,
}

/// Prune dialog state
#[derive(Debug, Clone)]
pub struct PruneDialogState {
    /// Whether to prune containers
    pub containers: bool,
    /// Whether to prune images
    pub images: bool,
    /// Whether to prune volumes
    pub volumes: bool,
    /// Whether to prune networks
    pub networks: bool,
    /// Whether to prune build cache
    pub build_cache: bool,
    /// Currently selected option (0-4 for individual, 5 for everything)
    pub selected_index: usize,
}

impl Default for PruneDialogState {
    fn default() -> Self {
        Self {
            containers: false,
            images: true, // Default to pruning dangling images
            volumes: false,
            networks: false,
            build_cache: false,
            selected_index: 0,
        }
    }
}

/// Panel focus areas
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Sidebar,
    Main,
}

/// Notification message
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: uuid::Uuid,
    pub message: String,
    pub level: NotificationLevel,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    /// Create new app state
    pub fn new() -> Self {
        Self {
            current_tab: Tab::Containers,
            previous_tab: None,
            focused_panel: Panel::Sidebar,
            containers: vec![],
            selected_container: None,
            container_list_selected: 0,
            images: vec![],
            selected_image: None,
            image_list_selected: 0,
            volumes: vec![],
            selected_volume: None,
            volume_list_selected: 0,
            networks: vec![],
            selected_network: None,
            network_list_selected: 0,
            docker_connected: false,
            connection_info: ConnectionInfo::default(),
            disk_usage: None,
            terminal_size: (80, 24),
            show_help: false,
            notifications: vec![],
            log_view: None,
            stats_view: None,
            exec_view: None,
            detail_view: None,
            image_detail_view: None,
            prune_dialog: None,
            confirm_dialog: None,
            loading: false,
        }
    }

    /// Add a notification
    pub fn add_notification(&mut self, message: impl Into<String>, level: NotificationLevel) {
        let notification = Notification {
            id: uuid::Uuid::new_v4(),
            message: message.into(),
            level,
            timestamp: Utc::now(),
        };
        self.notifications.push(notification);

        // Keep only last 10 notifications
        if self.notifications.len() > 10 {
            self.notifications.remove(0);
        }
    }

    /// Clear old notifications (older than threshold)
    pub fn clear_old_notifications(&mut self, max_age_seconds: i64) {
        let cutoff = Utc::now() - chrono::Duration::seconds(max_age_seconds);
        self.notifications.retain(|n| n.timestamp > cutoff);
    }

    /// Update images list
    pub fn update_images(&mut self, images: Vec<ImageSummary>) {
        self.images = images;
        // Adjust selection if needed
        if !self.images.is_empty() {
            if self.image_list_selected >= self.images.len() {
                self.image_list_selected = self.images.len() - 1;
            }
            self.selected_image = Some(self.images[self.image_list_selected].id.clone());
        } else {
            self.image_list_selected = 0;
            self.selected_image = None;
        }
    }

    /// Navigate to next image in list
    pub fn next_image(&mut self) {
        if self.images.is_empty() {
            return;
        }
        self.image_list_selected = (self.image_list_selected + 1) % self.images.len();
        self.selected_image = Some(self.images[self.image_list_selected].id.clone());
    }

    /// Navigate to previous image in list
    pub fn previous_image(&mut self) {
        if self.images.is_empty() {
            return;
        }
        if self.image_list_selected == 0 {
            self.image_list_selected = self.images.len() - 1;
        } else {
            self.image_list_selected -= 1;
        }
        self.selected_image = Some(self.images[self.image_list_selected].id.clone());
    }

    /// Update containers list
    pub fn update_containers(&mut self, containers: Vec<ContainerSummary>) {
        self.containers = containers;
        // Adjust selection if needed
        if !self.containers.is_empty() {
            if self.container_list_selected >= self.containers.len() {
                self.container_list_selected = self.containers.len() - 1;
            }
            self.selected_container =
                Some(self.containers[self.container_list_selected].id.clone());
        } else {
            self.container_list_selected = 0;
            self.selected_container = None;
        }
    }

    /// Navigate to next container in list
    pub fn next_container(&mut self) {
        if self.containers.is_empty() {
            return;
        }
        self.container_list_selected = (self.container_list_selected + 1) % self.containers.len();
        self.selected_container = Some(self.containers[self.container_list_selected].id.clone());
    }

    /// Navigate to previous container in list
    pub fn previous_container(&mut self) {
        if self.containers.is_empty() {
            return;
        }
        if self.container_list_selected == 0 {
            self.container_list_selected = self.containers.len() - 1;
        } else {
            self.container_list_selected -= 1;
        }
        self.selected_container = Some(self.containers[self.container_list_selected].id.clone());
    }

    /// Update volumes list
    pub fn update_volumes(&mut self, volumes: Vec<VolumeSummary>) {
        self.volumes = volumes;
        // Adjust selection if needed
        if !self.volumes.is_empty() {
            if self.volume_list_selected >= self.volumes.len() {
                self.volume_list_selected = self.volumes.len() - 1;
            }
            self.selected_volume = Some(self.volumes[self.volume_list_selected].name.clone());
        } else {
            self.volume_list_selected = 0;
            self.selected_volume = None;
        }
    }

    /// Navigate to next volume in list
    pub fn next_volume(&mut self) {
        if self.volumes.is_empty() {
            return;
        }
        self.volume_list_selected = (self.volume_list_selected + 1) % self.volumes.len();
        self.selected_volume = Some(self.volumes[self.volume_list_selected].name.clone());
    }

    /// Navigate to previous volume in list
    pub fn previous_volume(&mut self) {
        if self.volumes.is_empty() {
            return;
        }
        if self.volume_list_selected == 0 {
            self.volume_list_selected = self.volumes.len() - 1;
        } else {
            self.volume_list_selected -= 1;
        }
        self.selected_volume = Some(self.volumes[self.volume_list_selected].name.clone());
    }

    /// Update networks list
    pub fn update_networks(&mut self, networks: Vec<NetworkSummary>) {
        self.networks = networks;
        // Adjust selection if needed
        if !self.networks.is_empty() {
            if self.network_list_selected >= self.networks.len() {
                self.network_list_selected = self.networks.len() - 1;
            }
            self.selected_network = Some(self.networks[self.network_list_selected].id.clone());
        } else {
            self.network_list_selected = 0;
            self.selected_network = None;
        }
    }

    /// Update disk usage information
    pub fn update_disk_usage(&mut self, disk_usage: SystemDiskUsage) {
        self.disk_usage = Some(disk_usage);
    }

    /// Navigate to next network in list
    pub fn next_network(&mut self) {
        if self.networks.is_empty() {
            return;
        }
        self.network_list_selected = (self.network_list_selected + 1) % self.networks.len();
        self.selected_network = Some(self.networks[self.network_list_selected].id.clone());
    }

    /// Navigate to previous network in list
    pub fn previous_network(&mut self) {
        if self.networks.is_empty() {
            return;
        }
        if self.network_list_selected == 0 {
            self.network_list_selected = self.networks.len() - 1;
        } else {
            self.network_list_selected -= 1;
        }
        self.selected_network = Some(self.networks[self.network_list_selected].id.clone());
    }

    /// Set Docker connection status
    pub fn set_docker_connected(&mut self, connected: bool, info: ConnectionInfo) {
        self.docker_connected = connected;
        self.connection_info = info;
    }

    /// Open log view for a container
    pub fn open_log_view(&mut self, container_id: String, container_name: String) {
        self.log_view = Some(LogViewState {
            container_id,
            container_name,
            logs: vec![],
            scroll_offset: 0,
            follow: true,
            max_lines: 1000,
            search_pattern: None,
            search_matches: vec![],
            current_match: None,
            show_search_input: false,
            level_filter: LogLevelFilter::All,
            time_filter: None,
            show_time_input: false,
        });
    }

    /// Close log view
    pub fn close_log_view(&mut self) {
        self.log_view = None;
    }

    /// Add log entry to log view
    pub fn add_log_entry(&mut self, entry: crate::docker::LogEntry) {
        if let Some(log_view) = &mut self.log_view {
            log_view.logs.push(entry);
            // Trim to max lines
            if log_view.logs.len() > log_view.max_lines {
                log_view.logs.remove(0);
                if log_view.scroll_offset > 0 {
                    log_view.scroll_offset -= 1;
                }
            }
            // Auto-scroll if following
            if log_view.follow {
                log_view.scroll_offset = log_view.logs.len().saturating_sub(1);
            }
        }
    }

    /// Scroll up in log view
    pub fn scroll_logs_up(&mut self, amount: usize) {
        if let Some(log_view) = &mut self.log_view {
            log_view.scroll_offset = log_view.scroll_offset.saturating_sub(amount);
            log_view.follow = false; // Disable follow when manually scrolling
        }
    }

    /// Scroll down in log view
    pub fn scroll_logs_down(&mut self, amount: usize) {
        if let Some(log_view) = &mut self.log_view {
            let max_offset = log_view.logs.len().saturating_sub(1);
            log_view.scroll_offset = (log_view.scroll_offset + amount).min(max_offset);
            // Re-enable follow if at bottom
            if log_view.scroll_offset >= max_offset {
                log_view.follow = true;
            }
        }
    }

    /// Toggle follow mode
    pub fn toggle_log_follow(&mut self) {
        if let Some(log_view) = &mut self.log_view {
            log_view.follow = !log_view.follow;
            if log_view.follow {
                // Jump to bottom when enabling follow
                log_view.scroll_offset = log_view.logs.len().saturating_sub(1);
            }
        }
    }

    /// Show search input in log view
    pub fn show_log_search(&mut self) {
        if let Some(log_view) = &mut self.log_view {
            log_view.show_search_input = true;
            log_view.follow = false; // Disable follow when searching
        }
    }

    /// Hide search input in log view
    pub fn hide_log_search(&mut self) {
        if let Some(log_view) = &mut self.log_view {
            log_view.show_search_input = false;
        }
    }

    /// Set search pattern and find matches
    pub fn set_log_search(&mut self, pattern: &str) {
        if let Some(log_view) = &mut self.log_view {
            if pattern.is_empty() {
                log_view.search_pattern = None;
                log_view.search_matches.clear();
                log_view.current_match = None;
                return;
            }

            log_view.search_pattern = Some(pattern.to_string());
            log_view.search_matches.clear();

            // Find all matching log entries (case-insensitive)
            let pattern_lower = pattern.to_lowercase();
            for (i, entry) in log_view.logs.iter().enumerate() {
                if entry.message.to_lowercase().contains(&pattern_lower) {
                    log_view.search_matches.push(i);
                }
            }

            // Set current match to first one
            log_view.current_match = if log_view.search_matches.is_empty() {
                None
            } else {
                Some(0)
            };

            // Scroll to first match if found
            if let Some(&match_idx) = log_view.search_matches.first() {
                log_view.scroll_offset = match_idx;
            }
        }
    }

    /// Clear search pattern
    pub fn clear_log_search(&mut self) {
        if let Some(log_view) = &mut self.log_view {
            log_view.search_pattern = None;
            log_view.search_matches.clear();
            log_view.current_match = None;
        }
    }

    /// Jump to next search match
    pub fn next_search_match(&mut self) {
        if let Some(log_view) = &mut self.log_view {
            if let Some(current) = log_view.current_match {
                let next = (current + 1) % log_view.search_matches.len();
                log_view.current_match = Some(next);
                log_view.scroll_offset = log_view.search_matches[next];
            }
        }
    }

    /// Jump to previous search match
    pub fn prev_search_match(&mut self) {
        if let Some(log_view) = &mut self.log_view {
            if let Some(current) = log_view.current_match {
                let prev = if current == 0 {
                    log_view.search_matches.len() - 1
                } else {
                    current - 1
                };
                log_view.current_match = Some(prev);
                log_view.scroll_offset = log_view.search_matches[prev];
            }
        }
    }

    /// Set log level filter
    pub fn set_log_level_filter(&mut self, filter: LogLevelFilter) {
        if let Some(log_view) = &mut self.log_view {
            log_view.level_filter = filter;
            // Reset scroll to top when changing filter
            log_view.scroll_offset = 0;
        }
    }

    /// Clear log level filter (set to All)
    pub fn clear_log_level_filter(&mut self) {
        self.set_log_level_filter(LogLevelFilter::All);
    }

    /// Show time filter input
    pub fn show_time_filter(&mut self) {
        if let Some(log_view) = &mut self.log_view {
            log_view.show_time_input = true;
            log_view.follow = false;
        }
    }

    /// Hide time filter input
    pub fn hide_time_filter(&mut self) {
        if let Some(log_view) = &mut self.log_view {
            log_view.show_time_input = false;
        }
    }

    /// Set time filter (e.g., "5m" for 5 minutes, "1h" for 1 hour)
    pub fn set_time_filter(&mut self, input: &str) {
        if let Some(log_view) = &mut self.log_view {
            let now = chrono::Utc::now();

            let cutoff = if input.ends_with('m') {
                input
                    .trim_end_matches('m')
                    .parse::<i64>()
                    .ok()
                    .map(|mins| now - chrono::Duration::minutes(mins))
            } else if input.ends_with('h') {
                input
                    .trim_end_matches('h')
                    .parse::<i64>()
                    .ok()
                    .map(|hours| now - chrono::Duration::hours(hours))
            } else if input.ends_with('d') {
                input
                    .trim_end_matches('d')
                    .parse::<i64>()
                    .ok()
                    .map(|days| now - chrono::Duration::days(days))
            } else {
                // Try to parse as ISO timestamp
                chrono::DateTime::parse_from_rfc3339(input)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            };

            log_view.time_filter = cutoff;
            log_view.scroll_offset = 0;
        }
    }

    /// Clear time filter
    pub fn clear_time_filter(&mut self) {
        if let Some(log_view) = &mut self.log_view {
            log_view.time_filter = None;
            log_view.scroll_offset = 0;
        }
    }

    // ==================== Stats View Methods ====================

    /// Open stats view for a container
    pub fn open_stats_view(&mut self, container_id: String, container_name: String) {
        self.stats_view = Some(StatsViewState {
            container_id,
            container_name,
            stats: None,
            follow: true,
            error: None,
        });
    }

    /// Close stats view
    pub fn close_stats_view(&mut self) {
        self.stats_view = None;
    }

    /// Update stats in stats view
    pub fn update_stats(&mut self, stats: crate::docker::StatsEntry) {
        if let Some(stats_view) = &mut self.stats_view {
            stats_view.stats = Some(stats);
            stats_view.error = None;
        }
    }

    /// Set stats error
    pub fn set_stats_error(&mut self, error: String) {
        if let Some(stats_view) = &mut self.stats_view {
            stats_view.error = Some(error);
        }
    }

    /// Toggle stats follow mode
    pub fn toggle_stats_follow(&mut self) {
        if let Some(stats_view) = &mut self.stats_view {
            stats_view.follow = !stats_view.follow;
        }
    }

    // ==================== Exec View Methods ====================

    /// Open exec view for a container
    pub fn open_exec_view(&mut self, container_id: String, container_name: String) {
        self.exec_view = Some(ExecViewState {
            container_id,
            container_name,
            focus: true,
            status: "Starting".to_string(),
            screen_lines: vec![],
            cursor: None,
            attach: false,
        });
        // Avoid stacking bottom panels
        self.stats_view = None;
    }

    /// Close exec view
    pub fn close_exec_view(&mut self) {
        self.exec_view = None;
    }

    /// Mark the exec pane as an attach session
    pub fn set_exec_attach(&mut self, attach: bool) {
        if let Some(exec_view) = &mut self.exec_view {
            exec_view.attach = attach;
        }
    }

    /// Toggle exec focus
    pub fn toggle_exec_focus(&mut self) {
        if let Some(exec_view) = &mut self.exec_view {
            exec_view.focus = !exec_view.focus;
        }
    }

    /// Update exec screen buffer
    pub fn update_exec_screen(&mut self, lines: Vec<String>, status: Option<String>) {
        if let Some(exec_view) = &mut self.exec_view {
            exec_view.screen_lines = lines;
            if let Some(status) = status {
                exec_view.status = status;
            }
        }
    }

    /// Set exec status text
    pub fn set_exec_status(&mut self, status: impl Into<String>) {
        if let Some(exec_view) = &mut self.exec_view {
            exec_view.status = status.into();
        }
    }

    /// Set exec cursor position
    pub fn set_exec_cursor(&mut self, cursor: Option<(u16, u16)>) {
        if let Some(exec_view) = &mut self.exec_view {
            exec_view.cursor = cursor;
        }
    }

    // ==================== Detail View Methods ====================

    /// Open detail view for a container
    pub fn open_detail_view(&mut self, container_id: String, container_name: String) {
        self.detail_view = Some(DetailViewState {
            container_id,
            container_name,
            details: None,
            scroll_offset: 0,
            search: ViewSearch::default(),
        });
    }

    /// Close detail view
    pub fn close_detail_view(&mut self) {
        self.detail_view = None;
    }

    /// Set detail view content
    pub fn set_detail_view_content(&mut self, details: crate::docker::ContainerDetails) {
        if let Some(detail_view) = &mut self.detail_view {
            detail_view.details = Some(details);
        }
    }

    /// Scroll up in detail view
    pub fn scroll_detail_view_up(&mut self, amount: usize) {
        if let Some(detail_view) = &mut self.detail_view {
            detail_view.scroll_offset = detail_view.scroll_offset.saturating_sub(amount);
        }
    }

    /// Scroll down in detail view
    pub fn scroll_detail_view_down(&mut self, amount: usize) {
        if let Some(detail_view) = &mut self.detail_view {
            detail_view.scroll_offset += amount;
        }
    }

    // ==================== Image Detail View Methods ====================

    /// Open image detail view
    pub fn open_image_detail_view(&mut self, image_id: String, image_name: String) {
        self.image_detail_view = Some(ImageDetailViewState {
            image_id,
            image_name,
            details: None,
            scroll_offset: 0,
            search: ViewSearch::default(),
        });
    }

    /// Close image detail view
    pub fn close_image_detail_view(&mut self) {
        self.image_detail_view = None;
    }

    /// Set image detail view content
    pub fn set_image_detail_view_content(&mut self, details: crate::docker::ImageDetails) {
        if let Some(image_detail_view) = &mut self.image_detail_view {
            image_detail_view.details = Some(details);
        }
    }

    /// Scroll up in image detail view
    pub fn scroll_image_detail_view_up(&mut self, amount: usize) {
        if let Some(image_detail_view) = &mut self.image_detail_view {
            image_detail_view.scroll_offset =
                image_detail_view.scroll_offset.saturating_sub(amount);
        }
    }

    /// Scroll down in image detail view
    pub fn scroll_image_detail_view_down(&mut self, amount: usize) {
        if let Some(image_detail_view) = &mut self.image_detail_view {
            image_detail_view.scroll_offset += amount;
        }
    }

    /// Open prune dialog
    pub fn open_prune_dialog(&mut self) {
        self.prune_dialog = Some(PruneDialogState::default());
    }

    /// Close prune dialog
    pub fn close_prune_dialog(&mut self) {
        self.prune_dialog = None;
    }

    /// Navigate up in prune dialog
    pub fn prune_dialog_prev(&mut self) {
        if let Some(dialog) = &mut self.prune_dialog {
            if dialog.selected_index > 0 {
                dialog.selected_index -= 1;
            }
        }
    }

    /// Navigate down in prune dialog
    pub fn prune_dialog_next(&mut self) {
        if let Some(dialog) = &mut self.prune_dialog {
            if dialog.selected_index < 4 {
                dialog.selected_index += 1;
            }
        }
    }

    /// Toggle current option in prune dialog
    pub fn prune_dialog_toggle(&mut self) {
        if let Some(dialog) = &mut self.prune_dialog {
            match dialog.selected_index {
                0 => dialog.containers = !dialog.containers,
                1 => dialog.images = !dialog.images,
                2 => dialog.volumes = !dialog.volumes,
                3 => dialog.networks = !dialog.networks,
                4 => {
                    // Toggle everything
                    let all =
                        !(dialog.containers && dialog.images && dialog.volumes && dialog.networks);
                    dialog.containers = all;
                    dialog.images = all;
                    dialog.volumes = all;
                    dialog.networks = all;
                }
                _ => {}
            }
        }
    }

    /// Get prune options from dialog
    pub fn get_prune_options(&self) -> Option<PruneOptions> {
        self.prune_dialog.as_ref().map(|dialog| PruneOptions {
            containers: dialog.containers,
            images: dialog.images,
            volumes: dialog.volumes,
            networks: dialog.networks,
            build_cache: dialog.build_cache,
        })
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert_eq!(state.current_tab, Tab::Containers);
        assert!(state.containers.is_empty());
        assert!(!state.docker_connected);
    }

    #[test]
    fn test_add_notification() {
        let mut state = AppState::default();
        state.add_notification("Test message", NotificationLevel::Info);

        assert_eq!(state.notifications.len(), 1);
        assert_eq!(state.notifications[0].message, "Test message");
    }

    #[test]
    fn test_notification_limit() {
        let mut state = AppState::default();

        // Add 15 notifications
        for i in 0..15 {
            state.add_notification(format!("Message {}", i), NotificationLevel::Info);
        }

        // Should only keep last 10
        assert_eq!(state.notifications.len(), 10);
    }

    #[test]
    fn test_update_containers() {
        let mut state = AppState::default();
        let containers = vec![ContainerSummary::default()];

        state.update_containers(containers);
        assert_eq!(state.containers.len(), 1);
    }

    #[test]
    fn exec_view_open_close_toggle_focus() {
        let mut state = AppState::new();
        state.open_exec_view("abc".into(), "web".into());
        assert!(state.exec_view.is_some());
        let exec_view = state.exec_view.as_ref().unwrap();
        assert!(exec_view.focus);
        assert_eq!(exec_view.status, "Starting");
        state.toggle_exec_focus();
        assert!(!state.exec_view.as_ref().unwrap().focus);
        state.close_exec_view();
        assert!(state.exec_view.is_none());
    }

    #[test]
    fn exec_view_set_status() {
        let mut state = AppState::new();
        state.open_exec_view("abc".into(), "web".into());
        state.set_exec_status("Starting |");
        let exec_view = state.exec_view.as_ref().unwrap();
        assert_eq!(exec_view.status, "Starting |");
    }

    #[test]
    fn exec_view_set_cursor() {
        let mut state = AppState::new();
        state.open_exec_view("abc".into(), "web".into());
        state.set_exec_cursor(Some((2, 4)));
        let exec_view = state.exec_view.as_ref().unwrap();
        assert_eq!(exec_view.cursor, Some((2, 4)));
    }
}

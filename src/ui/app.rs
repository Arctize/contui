//! UI Application logic

use crate::ui::theme;
use std::borrow::Cow;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use tracing::{debug, info};

use crate::core::{ConfirmAction, ContainerState, Tab, UiAction};
use crate::docker::format_bytes_size;
use crate::exec::input::encode_key_event;
use crate::state::AppState;
use crate::ui::components::ContainerListWidget;

/// UI Application controller
pub struct UiApp {
    pub state: AppState,
    pub should_quit: bool,
}

impl UiApp {
    /// Create a new UI app
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            should_quit: false,
        }
    }

    /// Handle a terminal event and return any action to execute
    pub fn handle_event(&mut self, event: Event) -> UiAction {
        match event {
            Event::Key(key_event) => self.handle_key_event(key_event),
            Event::Resize(width, height) => {
                debug!("Terminal resized to {}x{}", width, height);
                self.state.terminal_size = (width, height);
                UiAction::None
            }
            _ => UiAction::None,
        }
    }

    /// Handle keyboard events and return any action to execute
    fn handle_key_event(&mut self, key: KeyEvent) -> UiAction {
        // Only handle key press events (not release or repeat)
        if key.kind != KeyEventKind::Press {
            return UiAction::None;
        }

        // If confirmation dialog is showing
        if self.state.confirm_dialog.is_some() {
            return self.handle_confirmation_key(key);
        }

        // If help is showing, any key closes it (except when toggling help)
        if self.state.show_help && key.code != KeyCode::Char('?') && key.code != KeyCode::Char('h')
        {
            self.state.show_help = false;
            return UiAction::None;
        }

        // If log view is active, handle log view keys (log view is modal, blocks everything)
        if self.state.log_view.is_some() {
            return self.handle_log_view_key(key);
        }

        // If detail view is active, handle detail view keys (modal, blocks everything)
        if self.state.detail_view.is_some() {
            return self.handle_detail_view_key(key);
        }

        // If image detail view is active, handle image detail view keys (modal, blocks everything)
        if self.state.image_detail_view.is_some() {
            return self.handle_image_detail_view_key(key);
        }

        // If prune dialog is active, handle prune dialog keys (modal, blocks everything)
        if self.state.prune_dialog.is_some() {
            return self.handle_prune_dialog_key(key);
        }

        // If exec view is focused, route keys to exec (except Ctrl+E)
        if let Some(exec_view) = &self.state.exec_view {
            if exec_view.focus {
                if key.code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.state.toggle_exec_focus();
                    return UiAction::None;
                }
                if let Some(bytes) = encode_key_event(key) {
                    return UiAction::ExecInput(bytes);
                }
                return UiAction::None;
            }
        }

        // Always allow immediate quit
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            info!("Ctrl+C pressed");
            self.should_quit = true;
            return UiAction::Quit;
        }

        // Global key handlers
        match key.code {
            // Quit
            KeyCode::Char('q') if key.modifiers.is_empty() => {
                info!("Quit key pressed");
                self.should_quit = true;
                UiAction::Quit
            }

            // Tab switching with number keys
            KeyCode::Char('1') => {
                self.switch_tab(Tab::Containers);
                UiAction::None
            }
            KeyCode::Char('2') => {
                self.switch_tab(Tab::Images);
                UiAction::None
            }
            KeyCode::Char('3') => {
                self.switch_tab(Tab::Volumes);
                UiAction::None
            }
            KeyCode::Char('4') => {
                self.switch_tab(Tab::Networks);
                UiAction::None
            }
            KeyCode::Char('5') => {
                self.switch_tab(Tab::Compose);
                UiAction::None
            }
            KeyCode::Char('6') => {
                self.switch_tab(Tab::System);
                UiAction::None
            }

            // Tab switching with arrow keys
            KeyCode::Right => {
                self.next_tab();
                UiAction::None
            }
            KeyCode::Left => {
                self.previous_tab();
                UiAction::None
            }

            // Navigation between panels
            KeyCode::Tab => {
                self.next_panel();
                UiAction::None
            }
            KeyCode::BackTab => {
                self.previous_panel();
                UiAction::None
            }

            // Container actions (when on Containers tab) - must come before unguarded 'k'
            KeyCode::Char('s') if self.state.current_tab == Tab::Containers => {
                self.handle_start_stop_action()
            }
            KeyCode::Char('r') if self.state.current_tab == Tab::Containers => {
                self.handle_restart_action()
            }
            KeyCode::Char('p') if self.state.current_tab == Tab::Containers => {
                self.handle_pause_action()
            }
            KeyCode::Char('k')
                if self.state.current_tab == Tab::Containers && key.modifiers.is_empty() =>
            {
                self.handle_kill_action()
            }
            KeyCode::Char('d') if self.state.current_tab == Tab::Containers => {
                self.handle_remove_action()
            }
            KeyCode::Char('l') if self.state.current_tab == Tab::Containers => {
                self.handle_logs_action()
            }
            KeyCode::Char('m') if self.state.current_tab == Tab::Containers => {
                self.handle_stats_action()
            }
            KeyCode::Char('i') if self.state.current_tab == Tab::Containers => {
                self.handle_inspect_action()
            }
            KeyCode::Char('x') if self.state.current_tab == Tab::Containers => {
                self.handle_exec_action()
            }
            // Toggle stats follow when stats panel is visible
            KeyCode::Char('f') if self.state.stats_view.is_some() => {
                self.state.toggle_stats_follow();
                UiAction::None
            }
            // Toggle exec focus when exec view is visible
            KeyCode::Char('e')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.state.exec_view.is_some() =>
            {
                self.state.toggle_exec_focus();
                UiAction::None
            }

            // Image actions (when on Images tab)
            KeyCode::Char('d') if self.state.current_tab == Tab::Images => {
                self.handle_image_remove_action()
            }
            KeyCode::Char('p') if self.state.current_tab == Tab::Images => {
                self.handle_image_prune_action()
            }
            KeyCode::Char('i') if self.state.current_tab == Tab::Images => {
                self.handle_image_inspect_action()
            }

            // Volume actions (when on Volumes tab)
            KeyCode::Char('d') if self.state.current_tab == Tab::Volumes => {
                self.handle_volume_remove_action()
            }
            KeyCode::Char('p') if self.state.current_tab == Tab::Volumes => {
                self.handle_volume_prune_action()
            }

            // Network actions (when on Networks tab)
            KeyCode::Char('d') if self.state.current_tab == Tab::Networks => {
                self.handle_network_remove_action()
            }
            KeyCode::Char('p') if self.state.current_tab == Tab::Networks => {
                self.handle_network_prune_action()
            }
            KeyCode::Char('p') if self.state.current_tab == Tab::System => {
                self.state.open_prune_dialog();
                UiAction::None
            }

            // List navigation
            KeyCode::Char('j') => {
                match self.state.current_tab {
                    Tab::Containers => self.state.next_container(),
                    Tab::Images => self.state.next_image(),
                    Tab::Volumes => self.state.next_volume(),
                    Tab::Networks => self.state.next_network(),
                    _ => {}
                }
                UiAction::None
            }
            KeyCode::Char('k') => {
                match self.state.current_tab {
                    Tab::Containers => self.state.previous_container(),
                    Tab::Images => self.state.previous_image(),
                    Tab::Volumes => self.state.previous_volume(),
                    Tab::Networks => self.state.previous_network(),
                    _ => {}
                }
                UiAction::None
            }
            KeyCode::Up => {
                match self.state.current_tab {
                    Tab::Containers => self.state.previous_container(),
                    Tab::Images => self.state.previous_image(),
                    Tab::Volumes => self.state.previous_volume(),
                    Tab::Networks => self.state.previous_network(),
                    _ => self.previous_tab(),
                }
                UiAction::None
            }
            KeyCode::Down => {
                match self.state.current_tab {
                    Tab::Containers => self.state.next_container(),
                    Tab::Images => self.state.next_image(),
                    Tab::Volumes => self.state.next_volume(),
                    Tab::Networks => self.state.next_network(),
                    _ => self.next_tab(),
                }
                UiAction::None
            }

            // Help
            KeyCode::Char('?') | KeyCode::Char('h') if key.modifiers.is_empty() => {
                self.state.show_help = !self.state.show_help;
                UiAction::None
            }

            _ => {
                debug!("Unhandled key: {:?}", key);
                UiAction::None
            }
        }
    }

    /// Handle confirmation dialog keys
    fn handle_confirmation_key(&mut self, key: KeyEvent) -> UiAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                // Confirm action
                if let Some(confirm) = self.state.confirm_dialog.take() {
                    return confirm.action;
                }
                UiAction::None
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                // Cancel action
                self.state.confirm_dialog = None;
                UiAction::None
            }
            _ => UiAction::None,
        }
    }

    /// Handle log view keys
    fn handle_log_view_key(&mut self, key: KeyEvent) -> UiAction {
        // Handle search input mode first
        if let Some(ref log_view) = self.state.log_view {
            if log_view.show_search_input {
                return self.handle_log_search_key(key);
            }
        }

        match key.code {
            // Exit log view
            KeyCode::Char('q') | KeyCode::Esc => {
                self.state.close_log_view();
                UiAction::None
            }
            // Refresh logs
            KeyCode::Char('r') => {
                if let Some(ref log_view) = self.state.log_view {
                    let id = log_view.container_id.clone();
                    return UiAction::ShowContainerLogs(id);
                }
                UiAction::None
            }
            // Toggle follow mode
            KeyCode::Char('f') => {
                self.state.toggle_log_follow();
                UiAction::None
            }
            // Open search
            KeyCode::Char('/') => {
                self.state.show_log_search();
                UiAction::None
            }
            // Next search match
            KeyCode::Char('n') => {
                self.state.next_search_match();
                UiAction::None
            }
            // Previous search match
            KeyCode::Char('N') => {
                self.state.prev_search_match();
                UiAction::None
            }
            // Log level filters
            KeyCode::Char('0') => {
                self.state
                    .set_log_level_filter(crate::state::LogLevelFilter::All);
                UiAction::None
            }
            KeyCode::Char('1') => {
                self.state
                    .set_log_level_filter(crate::state::LogLevelFilter::Error);
                UiAction::None
            }
            KeyCode::Char('2') => {
                self.state
                    .set_log_level_filter(crate::state::LogLevelFilter::Warn);
                UiAction::None
            }
            KeyCode::Char('3') => {
                self.state
                    .set_log_level_filter(crate::state::LogLevelFilter::Info);
                UiAction::None
            }
            // Time filter
            KeyCode::Char('t') => {
                // Cycle through time filters: 5m -> 15m -> 1h -> all -> 5m
                if let Some(ref log_view) = self.state.log_view {
                    let new_filter = match log_view.time_filter {
                        None => Some("5m"),
                        Some(_) => {
                            let elapsed = chrono::Utc::now() - log_view.time_filter.unwrap();
                            let mins = elapsed.num_minutes();
                            if mins <= 5 {
                                Some("15m")
                            } else if mins <= 15 {
                                Some("1h")
                            } else {
                                None // clear filter
                            }
                        }
                    };
                    if let Some(filter) = new_filter {
                        self.state.set_time_filter(filter);
                    } else {
                        self.state.clear_time_filter();
                    }
                }
                UiAction::None
            }
            KeyCode::Char('T') => {
                self.state.clear_time_filter();
                UiAction::None
            }
            // Export logs to file
            KeyCode::Char('s') => UiAction::ExportLogs,
            // Scroll up
            KeyCode::Up | KeyCode::PageUp => {
                let amount = if key.code == KeyCode::PageUp { 10 } else { 1 };
                self.state.scroll_logs_up(amount);
                UiAction::None
            }
            // Scroll down
            KeyCode::Down | KeyCode::PageDown => {
                let amount = if key.code == KeyCode::PageDown { 10 } else { 1 };
                self.state.scroll_logs_down(amount);
                UiAction::None
            }
            // Go to top
            KeyCode::Home => {
                if let Some(ref mut log_view) = self.state.log_view {
                    log_view.scroll_offset = 0;
                    log_view.follow = false;
                }
                UiAction::None
            }
            // Go to bottom
            KeyCode::End => {
                if let Some(ref mut log_view) = self.state.log_view {
                    log_view.scroll_offset = log_view.logs.len().saturating_sub(1);
                    log_view.follow = true;
                }
                UiAction::None
            }
            _ => UiAction::None,
        }
    }

    /// The full screen area the overlays are laid out in
    fn screen_area(&self) -> ratatui::layout::Rect {
        let (width, height) =
            ratatui::crossterm::terminal::size().unwrap_or(self.state.terminal_size);
        ratatui::layout::Rect::new(0, 0, width, height)
    }

    /// Handle detail view keys
    fn handle_detail_view_key(&mut self, key: KeyEvent) -> UiAction {
        // Search (`/`, `n`, `N`, `Esc` to clear) comes first
        let area = self.screen_area();
        if let Some(view) = self.state.detail_view.as_mut() {
            if let Some(details) = view.details.as_ref() {
                let consumed = crate::ui::components::view_search::handle_key(
                    &mut view.search,
                    &mut view.scroll_offset,
                    key,
                    |q| crate::ui::components::detail_viewer::match_offsets(details, q, area),
                );
                if consumed {
                    return UiAction::None;
                }
            }
        }

        match key.code {
            // Exit detail view
            KeyCode::Char('q') | KeyCode::Esc => {
                self.state.close_detail_view();
                UiAction::None
            }
            // Scroll up
            KeyCode::Up | KeyCode::PageUp => {
                let amount = if key.code == KeyCode::PageUp { 10 } else { 3 };
                self.state.scroll_detail_view_up(amount);
                UiAction::None
            }
            // Scroll down
            KeyCode::Down | KeyCode::PageDown => {
                let amount = if key.code == KeyCode::PageDown { 10 } else { 3 };
                self.state.scroll_detail_view_down(amount);
                UiAction::None
            }
            // Home to top
            KeyCode::Home => {
                self.state.scroll_detail_view_up(9999);
                UiAction::None
            }
            // End to bottom
            KeyCode::End => {
                self.state.scroll_detail_view_down(9999);
                UiAction::None
            }
            _ => UiAction::None,
        }
    }

    /// Handle image detail view keys
    fn handle_image_detail_view_key(&mut self, key: KeyEvent) -> UiAction {
        // Search (`/`, `n`, `N`, `Esc` to clear) comes first
        let area = self.screen_area();
        if let Some(view) = self.state.image_detail_view.as_mut() {
            if let Some(details) = view.details.as_ref() {
                let consumed = crate::ui::components::view_search::handle_key(
                    &mut view.search,
                    &mut view.scroll_offset,
                    key,
                    |q| crate::ui::components::image_detail_viewer::match_offsets(details, q, area),
                );
                if consumed {
                    return UiAction::None;
                }
            }
        }

        match key.code {
            // Exit image detail view
            KeyCode::Char('q') | KeyCode::Esc => {
                self.state.close_image_detail_view();
                UiAction::None
            }
            // Scroll up
            KeyCode::Up | KeyCode::PageUp => {
                let amount = if key.code == KeyCode::PageUp { 10 } else { 3 };
                self.state.scroll_image_detail_view_up(amount);
                UiAction::None
            }
            // Scroll down
            KeyCode::Down | KeyCode::PageDown => {
                let amount = if key.code == KeyCode::PageDown { 10 } else { 3 };
                self.state.scroll_image_detail_view_down(amount);
                UiAction::None
            }
            // Home to top
            KeyCode::Home => {
                self.state.scroll_image_detail_view_up(9999);
                UiAction::None
            }
            // End to bottom
            KeyCode::End => {
                self.state.scroll_image_detail_view_down(9999);
                UiAction::None
            }
            _ => UiAction::None,
        }
    }

    /// Handle keys when prune dialog is open
    fn handle_prune_dialog_key(&mut self, key: KeyEvent) -> UiAction {
        match key.code {
            // Close dialog
            KeyCode::Char('q') | KeyCode::Esc => {
                self.state.close_prune_dialog();
                UiAction::None
            }
            // Navigate up
            KeyCode::Up => {
                self.state.prune_dialog_prev();
                UiAction::None
            }
            // Navigate down
            KeyCode::Down => {
                self.state.prune_dialog_next();
                UiAction::None
            }
            // Toggle checkbox with Space, confirm with Enter
            KeyCode::Char(' ') => {
                self.state.prune_dialog_toggle();
                UiAction::None
            }
            KeyCode::Enter => {
                // Check if any option is selected
                let has_selection = self
                    .state
                    .prune_dialog
                    .as_ref()
                    .is_some_and(|d| d.containers || d.images || d.volumes || d.networks);

                if has_selection {
                    // Confirm and execute prune - dialog will be closed after getting options
                    UiAction::PruneSystem
                } else {
                    // Nothing selected, just toggle current item
                    self.state.prune_dialog_toggle();
                    UiAction::None
                }
            }
            _ => UiAction::None,
        }
    }

    /// Handle keys when in log search input mode
    fn handle_log_search_key(&mut self, key: KeyEvent) -> UiAction {
        use ratatui::crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => {
                self.state.hide_log_search();
                self.state.clear_log_search();
                UiAction::None
            }
            KeyCode::Enter => {
                self.state.hide_log_search();
                UiAction::None
            }
            KeyCode::Backspace => {
                // Remove last character from search pattern
                if let Some(ref mut log_view) = self.state.log_view {
                    if let Some(ref pattern) = log_view.search_pattern {
                        let new_pattern: String = pattern
                            .chars()
                            .take(pattern.len().saturating_sub(1))
                            .collect();
                        if new_pattern.is_empty() {
                            self.state.clear_log_search();
                        } else {
                            self.state.set_log_search(&new_pattern);
                        }
                    }
                }
                UiAction::None
            }
            KeyCode::Char(c) => {
                // Add character to search pattern
                let new_pattern = if let Some(ref log_view) = self.state.log_view {
                    if let Some(ref pattern) = log_view.search_pattern {
                        format!("{}{}", pattern, c)
                    } else {
                        c.to_string()
                    }
                } else {
                    c.to_string()
                };
                self.state.set_log_search(&new_pattern);
                UiAction::None
            }
            _ => UiAction::None,
        }
    }

    /// Get the currently selected container ID
    fn selected_container_id(&self) -> Option<String> {
        self.state
            .containers
            .get(self.state.container_list_selected)
            .map(|c| c.id.clone())
    }

    /// Handle start/stop action
    fn handle_start_stop_action(&mut self) -> UiAction {
        if let Some(container) = self
            .state
            .containers
            .get(self.state.container_list_selected)
        {
            let id = container.id.clone();
            match container.state {
                ContainerState::Running => {
                    // Running -> Stop
                    UiAction::StopContainer(id)
                }
                ContainerState::Paused => {
                    // Paused -> Unpause
                    UiAction::UnpauseContainer(id)
                }
                _ => {
                    // Stopped/Exited -> Start
                    UiAction::StartContainer(id)
                }
            }
        } else {
            UiAction::None
        }
    }

    /// Handle restart action
    fn handle_restart_action(&mut self) -> UiAction {
        if let Some(id) = self.selected_container_id() {
            UiAction::RestartContainer(id)
        } else {
            UiAction::None
        }
    }

    /// Handle pause action
    fn handle_pause_action(&mut self) -> UiAction {
        if let Some(container) = self
            .state
            .containers
            .get(self.state.container_list_selected)
        {
            let id = container.id.clone();
            if container.state == ContainerState::Paused {
                UiAction::UnpauseContainer(id)
            } else {
                UiAction::PauseContainer(id)
            }
        } else {
            UiAction::None
        }
    }

    /// Handle kill action (with confirmation)
    fn handle_kill_action(&mut self) -> UiAction {
        if let Some(container) = self
            .state
            .containers
            .get(self.state.container_list_selected)
        {
            let name = container
                .names
                .first()
                .cloned()
                .unwrap_or_else(|| container.short_id.clone());
            let id = container.id.clone();

            self.state.confirm_dialog = Some(ConfirmAction {
                message: format!("Kill container '{}'?", name),
                action: UiAction::KillContainer(id),
            });
        }
        UiAction::None
    }

    /// Handle remove action (with confirmation)
    fn handle_remove_action(&mut self) -> UiAction {
        if let Some(container) = self
            .state
            .containers
            .get(self.state.container_list_selected)
        {
            let name = container
                .names
                .first()
                .cloned()
                .unwrap_or_else(|| container.short_id.clone());
            let id = container.id.clone();

            self.state.confirm_dialog = Some(ConfirmAction {
                message: format!("Remove container '{}'?", name),
                action: UiAction::RemoveContainer(id),
            });
        }
        UiAction::None
    }

    /// Handle logs action
    fn handle_logs_action(&mut self) -> UiAction {
        if let Some(container) = self
            .state
            .containers
            .get(self.state.container_list_selected)
        {
            let id = container.id.clone();

            // Return action to start log streaming (log view will be opened in handle_ui_action)
            UiAction::ShowContainerLogs(id)
        } else {
            UiAction::None
        }
    }

    /// Handle stats action
    fn handle_stats_action(&mut self) -> UiAction {
        if let Some(container) = self
            .state
            .containers
            .get(self.state.container_list_selected)
        {
            let id = container.id.clone();

            // Return action to show stats (stats view will be opened in handle_ui_action)
            UiAction::ShowContainerStats(id)
        } else {
            UiAction::None
        }
    }

    /// Handle exec action
    fn handle_exec_action(&mut self) -> UiAction {
        if let Some(container) = self
            .state
            .containers
            .get(self.state.container_list_selected)
        {
            let id = container.id.clone();
            if container.state == ContainerState::Running {
                UiAction::ExecContainer(id)
            } else {
                let name = container
                    .names
                    .first()
                    .cloned()
                    .unwrap_or_else(|| container.short_id.clone());
                self.state.confirm_dialog = Some(ConfirmAction {
                    message: format!("Start container '{}' to exec?", name),
                    action: UiAction::StartContainerAndExec(id),
                });
                UiAction::None
            }
        } else {
            UiAction::None
        }
    }
    /// Handle inspect action
    fn handle_inspect_action(&mut self) -> UiAction {
        if let Some(container) = self
            .state
            .containers
            .get(self.state.container_list_selected)
        {
            let id = container.id.clone();

            // Return action to show container details
            UiAction::ShowContainerDetails(id)
        } else {
            UiAction::None
        }
    }

    /// Handle image remove action (with confirmation)
    fn handle_image_remove_action(&mut self) -> UiAction {
        if let Some(image) = self.state.images.get(self.state.image_list_selected) {
            let name = if image.dangling {
                "<dangling>".to_string()
            } else {
                image
                    .repo_tags
                    .first()
                    .cloned()
                    .unwrap_or_else(|| image.short_id.clone())
            };
            let id = image.id.clone();

            self.state.confirm_dialog = Some(ConfirmAction {
                message: format!("Remove image '{}'?", name),
                action: UiAction::RemoveImage(id),
            });
        }
        UiAction::None
    }

    /// Handle image prune action (with confirmation)
    fn handle_image_prune_action(&mut self) -> UiAction {
        self.state.confirm_dialog = Some(ConfirmAction {
            message: "Remove all dangling images?".to_string(),
            action: UiAction::PruneImages,
        });
        UiAction::None
    }

    /// Handle image inspect action
    fn handle_image_inspect_action(&mut self) -> UiAction {
        if let Some(image) = self.state.images.get(self.state.image_list_selected) {
            let id = image.id.clone();
            UiAction::ShowImageDetails(id)
        } else {
            UiAction::None
        }
    }

    /// Handle volume remove action (with confirmation)
    fn handle_volume_remove_action(&mut self) -> UiAction {
        if let Some(volume) = self.state.volumes.get(self.state.volume_list_selected) {
            let name = volume.name.clone();

            self.state.confirm_dialog = Some(ConfirmAction {
                message: format!("Remove volume '{}'", name),
                action: UiAction::RemoveVolume(name),
            });
        }
        UiAction::None
    }

    /// Handle volume prune action (with confirmation)
    fn handle_volume_prune_action(&mut self) -> UiAction {
        self.state.confirm_dialog = Some(ConfirmAction {
            message: "Remove all unused volumes?".to_string(),
            action: UiAction::PruneVolumes,
        });
        UiAction::None
    }

    /// Handle network remove action (with confirmation)
    fn handle_network_remove_action(&mut self) -> UiAction {
        if let Some(network) = self.state.networks.get(self.state.network_list_selected) {
            let name = network.name.clone();
            let id = network.id.clone();

            self.state.confirm_dialog = Some(ConfirmAction {
                message: format!("Remove network '{}'", name),
                action: UiAction::RemoveNetwork(id),
            });
        }
        UiAction::None
    }

    /// Handle network prune action (with confirmation)
    fn handle_network_prune_action(&mut self) -> UiAction {
        self.state.confirm_dialog = Some(ConfirmAction {
            message: "Remove all unused networks?".to_string(),
            action: UiAction::PruneNetworks,
        });
        UiAction::None
    }

    /// Switch to a specific tab
    fn switch_tab(&mut self, tab: Tab) {
        if self.state.current_tab != tab {
            info!("Switching to tab: {:?}", tab);
            self.state.previous_tab = Some(self.state.current_tab);
            self.state.current_tab = tab;
        }
    }

    /// Move to next tab (circular)
    fn next_tab(&mut self) {
        let tabs = Tab::all();
        let current_idx = tabs
            .iter()
            .position(|t| *t == self.state.current_tab)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % tabs.len();
        self.switch_tab(tabs[next_idx]);
    }

    /// Move to previous tab (circular)
    fn previous_tab(&mut self) {
        let tabs = Tab::all();
        let current_idx = tabs
            .iter()
            .position(|t| *t == self.state.current_tab)
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            tabs.len() - 1
        } else {
            current_idx - 1
        };
        self.switch_tab(tabs[prev_idx]);
    }

    /// Move focus to next panel
    fn next_panel(&mut self) {
        debug!("Moving to next panel");
        // Will be implemented with panel management
    }

    /// Move focus to previous panel
    fn previous_panel(&mut self) {
        debug!("Moving to previous panel");
        // Will be implemented with panel management
    }

    /// Render the UI
    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        // Full screen clear first to prevent any ghost content
        frame.render_widget(Clear, area);

        // Create the main layout
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(3),    // Main content (at least 3 lines)
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // Render components
        self.render_header(frame, main_layout[0]);
        self.render_main_content(frame, main_layout[1]);
        self.render_footer(frame, main_layout[2]);

        // Render notification overlay (if any)
        if let Some(notif) = self.state.notifications.last() {
            self.render_notification(frame, area, notif);
        }

        // Render log viewer if active (on top of everything)
        if let Some(ref log_view) = self.state.log_view {
            crate::ui::components::log_viewer::render_log_viewer(frame, area, log_view);
        }

        // Render detail viewer if active (on top of everything)
        if let Some(ref detail_view) = self.state.detail_view {
            crate::ui::components::detail_viewer::render_detail_viewer(frame, area, detail_view);
        }

        // Render image detail viewer if active (on top of everything)
        if let Some(ref image_detail_view) = self.state.image_detail_view {
            crate::ui::components::image_detail_viewer::render_image_detail_viewer(
                frame,
                area,
                image_detail_view,
            );
        }

        // Render confirmation dialog if active
        if let Some(ref confirm) = self.state.confirm_dialog {
            self.render_confirmation_dialog(frame, area, confirm);
        }

        // Render prune dialog if active
        if self.state.prune_dialog.is_some() {
            self.render_prune_dialog(frame, area);
        }

        // Render help overlay if active (on top of everything except notifications)
        if self.state.show_help {
            self.render_help_overlay(frame, area);
        }
    }

    /// Render confirmation dialog
    fn render_confirmation_dialog(&self, frame: &mut Frame, area: Rect, confirm: &ConfirmAction) {
        // Create a centered popup (50% width, 20% height, min 8 lines)
        let popup_area = Self::centered_rect(50, 20, area);
        let popup_area = popup_area.intersection(Rect {
            x: popup_area.x,
            y: popup_area.y,
            width: popup_area.width,
            height: popup_area.height.max(8),
        });

        // Clear the background
        frame.render_widget(Clear, popup_area);

        // Create layout for dialog content
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Min(1),    // Message
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Buttons
            ])
            .split(popup_area);

        // Render dialog block with title
        let block = Block::default()
            .title(" ⚠️ Confirmation ")
            .title_style(theme::title().add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(theme::title());
        frame.render_widget(block, popup_area);

        // Render message
        let message = Paragraph::new(confirm.message.as_str())
            .style(theme::text())
            .wrap(Wrap { trim: true })
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(message, layout[0]);

        // Render buttons hint
        let buttons = Line::from(vec![
            Span::styled("[", theme::muted()),
            Span::styled(
                "y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] Yes   [", theme::muted()),
            Span::styled(
                "n",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("] No", theme::muted()),
        ]);
        let buttons_para = Paragraph::new(buttons).alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(buttons_para, layout[2]);
    }

    /// Render prune dialog
    fn render_prune_dialog(&self, frame: &mut Frame, area: Rect) {
        let dialog = match &self.state.prune_dialog {
            Some(d) => d,
            None => return,
        };

        // Create a centered popup (50% width, min 12 lines)
        let popup_area = Self::centered_rect(50, 40, area);

        // Clear the background
        frame.render_widget(Clear, popup_area);

        // Create layout for dialog content
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(1), // Title
                Constraint::Length(1), // Spacer
                Constraint::Length(5), // Options (5 items)
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Buttons hint
            ])
            .split(popup_area);

        // Render dialog block with title
        let block = Block::default()
            .title(" Prune Unused Resources ")
            .title_style(theme::title().add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(theme::title());
        frame.render_widget(block, popup_area);

        // Title
        let title = Paragraph::new("Select what to prune:");
        frame.render_widget(title, layout[0]);

        // Options
        let all_selected = dialog.containers && dialog.images && dialog.volumes && dialog.networks;
        let options = vec![
            (0, dialog.containers, "Containers (stopped)"),
            (1, dialog.images, "Images (dangling)"),
            (2, dialog.volumes, "Volumes (unused)"),
            (3, dialog.networks, "Networks (unused)"),
            (4, all_selected, "Everything (select all)"),
        ];

        let options_lines: Vec<Line> = options
            .into_iter()
            .map(|(idx, checked, label)| {
                let checkbox = if checked { "[✓]" } else { "[ ]" };
                let style = if idx == dialog.selected_index {
                    theme::accent().add_modifier(Modifier::BOLD)
                } else {
                    theme::text()
                };
                Line::from(vec![
                    Span::styled(format!("{} ", checkbox), style),
                    Span::styled(label, style),
                ])
            })
            .collect();

        let options_para = Paragraph::new(options_lines);
        frame.render_widget(options_para, layout[2]);

        // Buttons hint
        let hint = Line::from(vec![
            Span::styled("[", theme::muted()),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("]Confirm [", theme::muted()),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("]Cancel", theme::muted()),
        ]);
        let hint_para = Paragraph::new(hint).alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(hint_para, layout[4]);
    }

    /// Render notification toast
    fn render_notification(
        &self,
        frame: &mut Frame,
        area: Rect,
        notif: &crate::state::Notification,
    ) {
        use ratatui::widgets::{Clear, Paragraph};

        let color = match notif.level {
            crate::core::NotificationLevel::Info => Color::Blue,
            crate::core::NotificationLevel::Success => Color::Green,
            crate::core::NotificationLevel::Warning => Color::Yellow,
            crate::core::NotificationLevel::Error => Color::Red,
        };

        let text = format!(" {} ", notif.message);
        let width = text.len() as u16 + 2;
        let height = 3;

        // Position at top-right of screen
        let x = area.width.saturating_sub(width + 2);
        let y = 1;

        if x > 0 && area.height > height + y {
            let notif_area = Rect::new(x, y, width.min(area.width - x), height);

            frame.render_widget(Clear, notif_area);

            let paragraph = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(color)),
                )
                .style(Style::default().fg(color));

            frame.render_widget(paragraph, notif_area);
        }
    }

    /// Render the header
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let status_indicator = if self.state.docker_connected {
            ("●", Color::Green)
        } else {
            ("○", Color::Red)
        };

        let header_spans = vec![
            Span::styled(" 🐳 Contui ", theme::accent().add_modifier(Modifier::BOLD)),
            Span::styled(format!("v{} ", env!("CARGO_PKG_VERSION")), theme::muted()),
            Span::raw("| "),
            Span::styled(
                self.state.current_tab.name(),
                theme::title().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(status_indicator.0, Style::default().fg(status_indicator.1)),
            Span::styled(
                if self.state.docker_connected {
                    " Connected "
                } else {
                    " Disconnected "
                },
                Style::default().fg(status_indicator.1),
            ),
        ];

        let header = Line::from(header_spans);
        frame.render_widget(
            Paragraph::new(header).style(Style::default().bg(theme::bg())),
            area,
        );
    }

    /// Render the main content area
    fn render_main_content(&self, frame: &mut Frame, area: Rect) {
        // Create a layout for sidebar + main panel
        // Use min 12 chars for sidebar, max 20
        let sidebar_width = (area.width / 5).clamp(12, 20);

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(sidebar_width), // Sidebar
                Constraint::Min(0),                // Main panel
            ])
            .split(area);

        self.render_sidebar(frame, content_layout[0]);

        // Render tab-specific content
        match self.state.current_tab {
            Tab::Containers if !self.state.containers.is_empty() => {
                self.render_containers_split_view(frame, content_layout[1]);
            }
            Tab::Images if !self.state.images.is_empty() => {
                self.render_images_view(frame, content_layout[1]);
            }
            Tab::Volumes if !self.state.volumes.is_empty() => {
                self.render_volumes_view(frame, content_layout[1]);
            }
            Tab::Networks if !self.state.networks.is_empty() => {
                self.render_networks_view(frame, content_layout[1]);
            }
            _ => {
                self.render_main_panel(frame, content_layout[1]);
            }
        }
    }

    /// Render volumes view
    fn render_volumes_view(&self, frame: &mut Frame, area: Rect) {
        // Fill area with black background first
        let bg = Block::default().style(Style::default().bg(theme::bg()));
        frame.render_widget(bg, area);

        let mut widget = crate::ui::components::VolumeListWidget::new(self.state.volumes.clone());
        if !self.state.volumes.is_empty() {
            widget.set_selected(Some(self.state.volume_list_selected));
        }
        let table = widget.build_table();
        let mut table_state = ratatui::widgets::TableState::default();
        table_state.select(Some(self.state.volume_list_selected));
        frame.render_stateful_widget(table, area, &mut table_state);
    }

    /// Render networks view
    fn render_networks_view(&self, frame: &mut Frame, area: Rect) {
        // Fill area with black background first
        let bg = Block::default().style(Style::default().bg(theme::bg()));
        frame.render_widget(bg, area);

        let mut widget = crate::ui::components::NetworkListWidget::new(self.state.networks.clone());
        if !self.state.networks.is_empty() {
            widget.set_selected(Some(self.state.network_list_selected));
        }
        let table = widget.build_table();
        let mut table_state = ratatui::widgets::TableState::default();
        table_state.select(Some(self.state.network_list_selected));
        frame.render_stateful_widget(table, area, &mut table_state);
    }

    /// Render images view
    fn render_images_view(&self, frame: &mut Frame, area: Rect) {
        // Fill area with black background first
        let bg = Block::default().style(Style::default().bg(theme::bg()));
        frame.render_widget(bg, area);

        // Create image list widget
        let mut widget = crate::ui::components::ImageListWidget::new(self.state.images.clone());
        if !self.state.images.is_empty() {
            widget.set_selected(Some(self.state.image_list_selected));
        }
        let table = widget.build_table();
        let mut table_state = ratatui::widgets::TableState::default();
        table_state.select(Some(self.state.image_list_selected));
        frame.render_stateful_widget(table, area, &mut table_state);
    }

    /// Render containers in split view (list | detail)
    fn render_containers_split_view(&self, frame: &mut Frame, area: Rect) {
        use crate::ui::components::{ContainerDetailPanel, SplitLayout};

        // Fill area with black background first
        let bg = Block::default().style(Style::default().bg(theme::bg()));
        frame.render_widget(bg, area);

        // Split area: 60% for list, 40% for detail
        let (list_area, detail_area) = SplitLayout::horizontal_split(area, 60);

        // Check if exec or stats panel is open
        let show_exec = self.state.exec_view.is_some();
        let show_stats = self.state.stats_view.is_some();

        // Split list area vertically if exec or stats panel is shown
        let (table_area, bottom_area) = if show_exec {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(crate::ui::components::exec_viewer::EXEC_PANEL_HEIGHT),
                ])
                .split(list_area);
            (chunks[0], Some(chunks[1]))
        } else if show_stats {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(crate::ui::components::stats_viewer::STATS_PANEL_HEIGHT),
                ])
                .split(list_area);
            (chunks[0], Some(chunks[1]))
        } else {
            (list_area, None)
        };

        // Create container list
        let mut widget = ContainerListWidget::new(self.state.containers.clone());
        if !self.state.containers.is_empty() {
            widget.set_selected(Some(self.state.container_list_selected));
        }
        let table = widget.build_table();
        let mut table_state = ratatui::widgets::TableState::default();
        table_state.select(Some(self.state.container_list_selected));
        frame.render_stateful_widget(table, table_area, &mut table_state);

        // Render exec panel if visible
        if let (Some(bottom_area), Some(exec_view)) = (bottom_area, &self.state.exec_view) {
            crate::ui::components::exec_viewer::render_exec_panel(frame, bottom_area, exec_view);
        } else if let (Some(bottom_area), Some(stats_view)) = (bottom_area, &self.state.stats_view)
        {
            crate::ui::components::stats_viewer::render_stats_panel(frame, bottom_area, stats_view);
        }

        // Render detail panel for selected container
        if let Some(container) = self
            .state
            .containers
            .get(self.state.container_list_selected)
        {
            let detail = ContainerDetailPanel::draw(container);
            frame.render_widget(detail, detail_area);
        } else {
            // Empty state
            let empty = Paragraph::new("No container selected")
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(empty, detail_area);
        }
    }

    /// Render the sidebar with tabs
    fn render_sidebar(&self, frame: &mut Frame, area: Rect) {
        // Fill area with black background first
        let bg = Block::default().style(Style::default().bg(theme::bg()));
        frame.render_widget(bg, area);

        let mut lines = vec![];

        for tab in Tab::all() {
            let is_selected = self.state.current_tab == *tab;
            let shortcut = tab.shortcut();
            let name = tab.name();

            let style = if is_selected {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme::muted()
            };

            // Format: "▶ 1:Containers" or "  2:Images"
            let line_text = if is_selected {
                format!("▶ {}:{}", shortcut, name)
            } else {
                format!("  {}:{}", shortcut, name)
            };

            lines.push(Line::from(Span::styled(line_text, style)));
        }

        let sidebar = Paragraph::new(Text::from(lines)).block(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(theme::subtle()),
        );

        frame.render_widget(sidebar, area);
    }

    /// Render the main panel based on current tab
    fn render_main_panel(&self, frame: &mut Frame, area: Rect) {
        // Containers tab is handled by render_containers_split_view
        // when there are containers, otherwise fall through to simple view
        if self.state.current_tab == Tab::Containers && !self.state.containers.is_empty() {
            return;
        }
        self.render_simple_tab(frame, area);
    }

    /// Render simple tabs (Images, Volumes, etc.)
    fn render_simple_tab(&self, frame: &mut Frame, area: Rect) {
        // Fill area with black background first
        let bg = Block::default().style(Style::default().bg(theme::bg()));
        frame.render_widget(bg, area);

        let block = Block::default()
            .title(self.state.current_tab.name())
            .borders(Borders::ALL)
            .border_style(theme::subtle());

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        match self.state.current_tab {
            Tab::System => {
                self.render_system_tab(frame, inner_area);
            }
            _ => {
                let content = match self.state.current_tab {
                    Tab::Images => format!("Total: {}", self.state.images.len()),
                    Tab::Volumes => format!("Total: {}", self.state.volumes.len()),
                    Tab::Networks => format!("Total: {}", self.state.networks.len()),
                    Tab::Compose => "Docker Compose\n\nNot yet implemented".to_string(),
                    Tab::System => unreachable!(),
                    Tab::Containers => {
                        // This should only happen when containers list is empty
                        "No containers found.\n\nDocker may not be running or you may not have permissions."
                            .to_string()
                    }
                };
                let paragraph = Paragraph::new(content).wrap(Wrap { trim: true });
                frame.render_widget(paragraph, inner_area);
            }
        }
    }

    /// Render the System tab with disk usage
    fn render_system_tab(&self, frame: &mut Frame, area: Rect) {
        if !self.state.docker_connected {
            let text =
                Paragraph::new("Not connected to Docker\n\nPlease check your Docker daemon.")
                    .wrap(Wrap { trim: true });
            frame.render_widget(text, area);
            return;
        }

        // Create layout for system info and disk usage
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),  // System info section
                Constraint::Length(14), // Disk usage section
                Constraint::Min(1),     // Actions hint
            ])
            .split(area);

        // System info section
        let info_lines = vec![
            Line::from(vec![
                Span::styled("Docker Version:   ", theme::accent()),
                Span::raw(&self.state.connection_info.version),
            ]),
            Line::from(vec![
                Span::styled("API Version:      ", theme::accent()),
                Span::raw(&self.state.connection_info.api_version),
            ]),
            Line::from(vec![
                Span::styled("OS/Arch:          ", theme::accent()),
                Span::raw(format!(
                    "{}/{}",
                    self.state.connection_info.os, self.state.connection_info.arch
                )),
            ]),
        ];
        let info_para = Paragraph::new(info_lines).block(
            Block::default()
                .title(" System Information ")
                .borders(Borders::ALL),
        );
        frame.render_widget(info_para, layout[0]);

        // Disk usage section
        let disk_usage_lines = if let Some(usage) = &self.state.disk_usage {
            let total_reclaimable = usage.total_reclaimable();
            let reclaimable_pct = usage.reclaimable_percentage();

            vec![
                Line::from(vec![
                    Span::styled("Images:           ", theme::accent()),
                    Span::raw(format!(
                        "{}  (Reclaimable: {})",
                        format_bytes_size(usage.images.total),
                        format_bytes_size(usage.images.reclaimable)
                    )),
                ]),
                Line::from(vec![
                    Span::styled("Containers:       ", theme::accent()),
                    Span::raw(format!(
                        "{}  (Reclaimable: {})",
                        format_bytes_size(usage.containers.total),
                        format_bytes_size(usage.containers.reclaimable)
                    )),
                ]),
                Line::from(vec![
                    Span::styled("Local Volumes:    ", theme::accent()),
                    Span::raw(format!(
                        "{}  (Reclaimable: {})",
                        format_bytes_size(usage.volumes.total),
                        format_bytes_size(usage.volumes.reclaimable)
                    )),
                ]),
                Line::from(vec![
                    Span::styled("Build Cache:      ", theme::accent()),
                    Span::raw(format!(
                        "{}  (Reclaimable: {})",
                        format_bytes_size(usage.build_cache.total),
                        format_bytes_size(usage.build_cache.reclaimable)
                    )),
                ]),
                Line::from("─".repeat(50)),
                Line::from(vec![
                    Span::styled(
                        "Total:            ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format_bytes_size(usage.total_size()),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Total Reclaimable:", theme::title()),
                    Span::styled(
                        format!(
                            " {}  [{:.0}% can be freed]",
                            format_bytes_size(total_reclaimable),
                            reclaimable_pct
                        ),
                        theme::title(),
                    ),
                ]),
            ]
        } else {
            vec![Line::from("Loading disk usage...")]
        };

        let disk_para = Paragraph::new(disk_usage_lines)
            .block(Block::default().title(" Disk Usage ").borders(Borders::ALL));
        frame.render_widget(disk_para, layout[1]);

        // Actions hint
        let hint = Paragraph::new("Actions: [p]Prune unused resources").style(theme::muted());
        frame.render_widget(hint, layout[2]);
    }

    /// Render the footer
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let base_text = if self.state.log_view.is_some() {
            Cow::Borrowed(" [↑/↓]Scroll [r]Refresh [f]Follow [/]Search [s]Save [q]Close ")
        } else if self.state.detail_view.is_some() {
            Cow::Borrowed(" [↑/↓]Scroll [/]Search [q]Close ")
        } else if self.state.image_detail_view.is_some() {
            Cow::Borrowed(" [↑/↓]Scroll [/]Search [q]Close ")
        } else if self.state.confirm_dialog.is_some() {
            Cow::Borrowed(" [y]Yes [n]No ")
        } else if self.state.show_help {
            Cow::Borrowed(" Press any key to close help ")
        } else if self.state.current_tab == Tab::Containers && !self.state.containers.is_empty() {
            Cow::Borrowed(" [↑/↓]Select [s]Start [p]Pause [r]Restart [k]Kill [d]Delete [l]Logs [m]Stats [i]Inspect [x]Exec [?]Help [q]Quit ")
        } else if self.state.current_tab == Tab::Images && !self.state.images.is_empty() {
            Cow::Borrowed(" [↑/↓]Select [d]Delete [p]Prune [i]Inspect [?]Help [q]Quit ")
        } else if self.state.current_tab == Tab::Volumes && !self.state.volumes.is_empty() {
            Cow::Borrowed(" [↑/↓]Select [d]Delete [p]Prune [?]Help [q]Quit ")
        } else if self.state.current_tab == Tab::Networks && !self.state.networks.is_empty() {
            Cow::Borrowed(" [↑/↓]Select [d]Delete [p]Prune [?]Help [q]Quit ")
        } else {
            Cow::Borrowed(" [←/→ or 1-6]:Switch Tabs | [?]:Help | [q]:Quit ")
        };

        let help_text = if self.state.exec_view.is_some()
            && self.state.log_view.is_none()
            && self.state.detail_view.is_none()
            && self.state.image_detail_view.is_none()
            && self.state.confirm_dialog.is_none()
            && !self.state.show_help
        {
            Cow::Owned(format!("{base_text}[Ctrl+E]Focus "))
        } else {
            base_text
        };

        let style = if self.state.confirm_dialog.is_some() {
            theme::title().add_modifier(Modifier::BOLD)
        } else {
            theme::muted().bg(theme::bg())
        };

        let footer = Paragraph::new(help_text).style(style);
        frame.render_widget(footer, area);
    }

    /// Render help overlay
    fn render_help_overlay(&self, frame: &mut Frame, area: Rect) {
        // Create a centered popup (70% width, 80% height) - larger to fit all content
        let popup_area = Self::centered_rect(70, 80, area);

        // Clear the background
        frame.render_widget(Clear, popup_area);

        let help_text = r#"Keyboard Shortcuts

Navigation:
  ←/→ or 1-6       Switch between tabs
  Tab              Next panel / Shift+Tab Previous

Containers Tab:
  ↑/↓ or j/k       Select container
  s                Start/Stop container
  r                Restart container
  p                Pause/Unpause container
  k                Kill container
  d                Delete container
  l                View logs
  m                Toggle stats panel
  i                Inspect container (detailed info)
  x                Exec into container

Images Tab:
  ↑/↓ or j/k       Select image
  d                Delete image
  p                Prune dangling images
  i                Inspect image (detailed info)

Volumes Tab:
  ↑/↓ or j/k       Select volume
  d                Delete volume
  p                Prune unused volumes

Networks Tab:
  ↑/↓ or j/k       Select network
  d                Delete network
  p                Prune unused networks

System Tab:
  p                Prune unused resources (opens dialog)

Log View:
  ↑/↓ or PgUp/PgDn Scroll
  r                Refresh logs
  f                Toggle follow mode
  /                Search logs
  n/N              Next/previous match
  0-3              Filter: 0=All, 1=Error, 2=Warn, 3=Info
  t/T              Time filter: cycle/clear
  s                Save logs to file
  Home/End         Jump to top/bottom
  q or Esc         Close log view

Stats Panel (toggle with 'm'):
  f                Toggle live/pause updates

Exec Pane:
  Ctrl+E           Toggle focus between UI and exec

Detail View (inspect container):
  ↑/↓ or PgUp/PgDn Scroll
  Home/End         Jump to top/bottom
  q or Esc         Close detail view

Image Detail View (inspect image):
  ↑/↓ or PgUp/PgDn Scroll
  Home/End         Jump to top/bottom
  q or Esc         Close detail view

Global:
  q or Ctrl+C      Quit
  ? or h           Toggle help

Press any key to close...
"#;

        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title(" Help (Press any key to close) ")
                    .borders(Borders::ALL)
                    .border_style(theme::title()),
            )
            .style(theme::text())
            .wrap(Wrap { trim: true });

        frame.render_widget(help, popup_area);
    }

    /// Calculate centered rectangle for popups
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_ui_app_creation() {
        let state = AppState::default();
        let app = UiApp::new(state);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_quit_key() {
        let state = AppState::default();
        let mut app = UiApp::new(state);

        app.handle_key_event(KeyEvent::from(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn test_ctrl_c() {
        let state = AppState::default();
        let mut app = UiApp::new(state);

        app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn test_tab_switching_numbers() {
        let state = AppState::default();
        let mut app = UiApp::new(state);

        assert_eq!(app.state.current_tab, Tab::Containers);

        app.handle_key_event(KeyEvent::from(KeyCode::Char('2')));
        assert_eq!(app.state.current_tab, Tab::Images);

        app.handle_key_event(KeyEvent::from(KeyCode::Char('1')));
        assert_eq!(app.state.current_tab, Tab::Containers);
    }

    #[test]
    fn test_tab_switching_arrows() {
        let state = AppState::default();
        let mut app = UiApp::new(state);

        assert_eq!(app.state.current_tab, Tab::Containers);

        // Right arrow should go to Images
        app.handle_key_event(KeyEvent::from(KeyCode::Right));
        assert_eq!(app.state.current_tab, Tab::Images);

        // Right arrow should go to Volumes
        app.handle_key_event(KeyEvent::from(KeyCode::Right));
        assert_eq!(app.state.current_tab, Tab::Volumes);

        // Left arrow should go back to Images
        app.handle_key_event(KeyEvent::from(KeyCode::Left));
        assert_eq!(app.state.current_tab, Tab::Images);

        // Left arrow should go back to Containers
        app.handle_key_event(KeyEvent::from(KeyCode::Left));
        assert_eq!(app.state.current_tab, Tab::Containers);

        // Left arrow should wrap to System (last tab)
        app.handle_key_event(KeyEvent::from(KeyCode::Left));
        assert_eq!(app.state.current_tab, Tab::System);
    }

    #[test]
    fn test_help_toggle() {
        let state = AppState::default();
        let mut app = UiApp::new(state);

        assert!(!app.state.show_help);

        app.handle_key_event(KeyEvent::from(KeyCode::Char('?')));
        assert!(app.state.show_help);

        app.handle_key_event(KeyEvent::from(KeyCode::Char('?')));
        assert!(!app.state.show_help);
    }

    #[test]
    fn test_rendering() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        let state = AppState::default();
        let app = UiApp::new(state);

        terminal
            .draw(|f| {
                app.draw(f);
            })
            .unwrap();

        // Just verify it doesn't panic
    }

    #[test]
    fn exec_key_triggers_action() {
        let mut state = AppState::default();
        state.current_tab = Tab::Containers;
        state.containers = vec![crate::core::ContainerSummary {
            id: "abc".to_string(),
            state: crate::core::ContainerState::Running,
            ..Default::default()
        }];
        state.container_list_selected = 0;

        let mut app = UiApp::new(state);
        let action = app.handle_event(crossterm::event::Event::Key(
            crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('x')),
        ));

        assert!(matches!(action, UiAction::ExecContainer(_)));
    }

    #[test]
    fn exec_footer_includes_focus_hint() {
        let backend = TestBackend::new(200, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::default();
        state.current_tab = Tab::Containers;
        state.containers = vec![crate::core::ContainerSummary::default()];
        state.exec_view = Some(crate::state::ExecViewState {
            container_id: "id".into(),
            container_name: "web".into(),
            focus: true,
            status: "Starting".into(),
            screen_lines: vec![],
            cursor: None,
        });

        let app = UiApp::new(state);
        terminal.draw(|f| app.draw(f)).unwrap();

        let buffer = terminal.backend().buffer();
        let footer_line: String = (0..buffer.area.width)
            .filter_map(|x| {
                buffer
                    .cell((x, buffer.area.height - 1))
                    .map(|c| c.symbol().to_string())
            })
            .collect::<Vec<_>>()
            .join("");

        assert!(
            footer_line.contains("Ctrl+E")
                && footer_line.contains("Focus")
                && !footer_line.contains("Exec Focus"),
            "expected footer to include exec focus hint"
        );
    }
}

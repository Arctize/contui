//! Volume list widget

use crate::ui::theme;
use ratatui::{
    layout::Constraint,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table, TableState},
};

use crate::core::VolumeSummary;

/// Widget for displaying a list of Docker volumes
pub struct VolumeListWidget {
    volumes: Vec<VolumeSummary>,
    state: TableState,
}

impl VolumeListWidget {
    /// Create a new volume list widget
    pub fn new(volumes: Vec<VolumeSummary>) -> Self {
        let mut state = TableState::default();
        if !volumes.is_empty() {
            state.select(Some(0));
        }
        Self { volumes, state }
    }

    /// Update the volume list
    pub fn update_volumes(&mut self, volumes: Vec<VolumeSummary>) {
        let selected_id = self.selected_volume_id();

        self.volumes = volumes;

        if let Some(id) = selected_id {
            if let Some(idx) = self.volumes.iter().position(|v| v.name == id) {
                self.state.select(Some(idx));
            } else if !self.volumes.is_empty() {
                self.state.select(Some(0));
            }
        } else if !self.volumes.is_empty() && self.state.selected().is_none() {
            self.state.select(Some(0));
        }
    }

    /// Get the selected volume ID (name)
    pub fn selected_volume_id(&self) -> Option<String> {
        self.state
            .selected()
            .and_then(|idx| self.volumes.get(idx))
            .map(|v| v.name.clone())
    }

    /// Get the selected volume
    pub fn selected_volume(&self) -> Option<&VolumeSummary> {
        self.state.selected().and_then(|idx| self.volumes.get(idx))
    }

    /// Move selection down
    pub fn next(&mut self) {
        if self.volumes.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.volumes.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Move selection up
    pub fn previous(&mut self) {
        if self.volumes.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.volumes.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Get the number of volumes
    pub fn len(&self) -> usize {
        self.volumes.len()
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.volumes.is_empty()
    }

    /// Build the table widget
    pub fn build_table(&self) -> Table<'_> {
        let header = Row::new(vec!["NAME", "DRIVER", "SCOPE", "MOUNTPOINT"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(0);

        let rows: Vec<Row> = self
            .volumes
            .iter()
            .map(|v| {
                // Style for unused volumes
                let style = if v.in_use.is_empty() {
                    theme::warn()
                } else {
                    Style::default()
                };

                let scope = format!("{:?}", v.scope);

                // Truncate mountpoint if too long
                let mountpoint = if v.mountpoint.len() > 40 {
                    format!("...{}", &v.mountpoint[v.mountpoint.len() - 37..])
                } else {
                    v.mountpoint.clone()
                };

                Row::new(vec![
                    Line::from(Span::styled(v.name.clone(), style)),
                    Line::from(v.driver.clone()),
                    Line::from(scope.to_lowercase()),
                    Line::from(mountpoint),
                ])
            })
            .collect();

        Table::new(
            rows,
            [
                Constraint::Min(20),    // Name
                Constraint::Length(12), // Driver
                Constraint::Length(8),  // Scope
                Constraint::Min(20),    // Mountpoint
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(" Volumes ({}) ", self.volumes.len()))
                .borders(Borders::ALL),
        )
        .row_highlight_style(theme::selected())
        .highlight_symbol("▶ ")
    }

    /// Get the table state for rendering
    pub fn state(&mut self) -> &mut TableState {
        &mut self.state
    }

    /// Set the selected index
    pub fn set_selected(&mut self, index: Option<usize>) {
        self.state.select(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::VolumeScope;
    use chrono::Utc;

    fn create_test_volumes() -> Vec<VolumeSummary> {
        vec![
            VolumeSummary {
                name: "my-volume".to_string(),
                driver: "local".to_string(),
                mountpoint: "/var/lib/docker/volumes/my-volume/_data".to_string(),
                created_at: Utc::now(),
                status: Default::default(),
                labels: Default::default(),
                scope: VolumeScope::Local,
                options: Default::default(),
                in_use: vec!["container1".to_string()],
            },
            VolumeSummary {
                name: "unused-volume".to_string(),
                driver: "local".to_string(),
                mountpoint: "/var/lib/docker/volumes/unused/_data".to_string(),
                created_at: Utc::now(),
                status: Default::default(),
                labels: Default::default(),
                scope: VolumeScope::Local,
                options: Default::default(),
                in_use: vec![], // Unused
            },
        ]
    }

    #[test]
    fn test_volume_list_creation() {
        let volumes = create_test_volumes();
        let widget = VolumeListWidget::new(volumes);
        assert_eq!(widget.len(), 2);
        assert!(!widget.is_empty());
    }

    #[test]
    fn test_navigation() {
        let volumes = create_test_volumes();
        let mut widget = VolumeListWidget::new(volumes);

        assert_eq!(widget.state.selected(), Some(0));

        widget.next();
        assert_eq!(widget.state.selected(), Some(1));

        widget.next();
        assert_eq!(widget.state.selected(), Some(0)); // Wrap around
    }
}

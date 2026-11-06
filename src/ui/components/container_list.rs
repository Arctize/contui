//! Container list widget

use crate::ui::theme;
use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table, TableState},
};

use crate::core::{ContainerState, ContainerSummary};

/// Widget for displaying a list of containers
pub struct ContainerListWidget {
    containers: Vec<ContainerSummary>,
    state: TableState,
    /// Show the compose project at the start of each group
    grouped: bool,
}

impl ContainerListWidget {
    /// Create a new container list widget
    pub fn new(containers: Vec<ContainerSummary>) -> Self {
        let mut state = TableState::default();
        if !containers.is_empty() {
            state.select(Some(0));
        }
        Self {
            containers,
            state,
            grouped: false,
        }
    }

    /// Show a dim STACK column naming the compose project of each group
    pub fn with_grouping(mut self, grouped: bool) -> Self {
        self.grouped = grouped;
        self
    }

    /// Update the container list
    pub fn update_containers(&mut self, containers: Vec<ContainerSummary>) {
        // Preserve selection if possible
        let selected_id = self.selected_container_id();

        self.containers = containers;

        // Try to restore selection
        if let Some(id) = selected_id {
            if let Some(idx) = self.containers.iter().position(|c| c.id == id) {
                self.state.select(Some(idx));
            } else if !self.containers.is_empty() {
                self.state.select(Some(0));
            }
        } else if !self.containers.is_empty() && self.state.selected().is_none() {
            self.state.select(Some(0));
        }
    }

    /// Get the selected container ID
    pub fn selected_container_id(&self) -> Option<String> {
        self.state
            .selected()
            .and_then(|idx| self.containers.get(idx))
            .map(|c| c.id.clone())
    }

    /// Get the selected container
    pub fn selected_container(&self) -> Option<&ContainerSummary> {
        self.state
            .selected()
            .and_then(|idx| self.containers.get(idx))
    }

    /// Move selection down
    pub fn next(&mut self) {
        if self.containers.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.containers.len() - 1 {
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
        if self.containers.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.containers.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Get the number of containers
    pub fn len(&self) -> usize {
        self.containers.len()
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.containers.is_empty()
    }

    /// Build the table widget
    pub fn build_table(&self) -> Table<'_> {
        let grouped = self.grouped && self.containers.iter().any(|c| c.compose_project.is_some());
        let mut columns = vec!["ID", "NAME", "IMAGE", "STATUS", "PORTS"];
        if grouped {
            columns.insert(0, "STACK");
        }
        let header = Row::new(columns)
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(0);

        let rows: Vec<Row> = self
            .containers
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let ports = if c.ports.is_empty() {
                    "-".to_string()
                } else {
                    c.ports
                        .iter()
                        .map(|p| {
                            if let Some(public) = p.public_port {
                                format!("{}:{}", public, p.private_port)
                            } else {
                                format!("{}", p.private_port)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                let status_style = match c.state {
                    ContainerState::Running => Style::default().fg(Color::Green),
                    ContainerState::Paused => theme::warn(),
                    ContainerState::Exited | ContainerState::Dead => {
                        Style::default().fg(Color::Red)
                    }
                    _ => theme::muted(),
                };

                let mut cells = vec![
                    Line::from(c.short_id.clone()),
                    Line::from(c.names.first().cloned().unwrap_or_else(|| "-".to_string())),
                    Line::from(c.image.clone()),
                    Line::from(Span::styled(c.status.clone(), status_style)),
                    Line::from(ports),
                ];
                if grouped {
                    // Name the project once, on the first row of its group
                    let starts_group =
                        i == 0 || self.containers[i - 1].compose_project != c.compose_project;
                    let label = match (&c.compose_project, starts_group) {
                        (Some(project), true) => project.clone(),
                        _ => String::new(),
                    };
                    cells.insert(
                        0,
                        Line::from(Span::styled(
                            label,
                            Style::default().add_modifier(Modifier::DIM),
                        )),
                    );
                }
                Row::new(cells)
            })
            .collect();

        let mut widths = vec![
            Constraint::Length(12), // ID
            Constraint::Min(10),    // Name
            Constraint::Min(15),    // Image
            Constraint::Length(20), // Status
            Constraint::Min(15),    // Ports
        ];
        if grouped {
            widths.insert(0, Constraint::Length(14)); // Stack
        }

        Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .title(format!(" Containers ({}) ", self.containers.len()))
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

    fn create_test_containers() -> Vec<ContainerSummary> {
        vec![
            ContainerSummary {
                id: "abc123def456".to_string(),
                short_id: "abc123".to_string(),
                names: vec!["web".to_string()],
                image: "nginx:latest".to_string(),
                state: ContainerState::Running,
                status: "Up 2 hours".to_string(),
                ..Default::default()
            },
            ContainerSummary {
                id: "def789ghi012".to_string(),
                short_id: "def789".to_string(),
                names: vec!["db".to_string()],
                image: "postgres:14".to_string(),
                state: ContainerState::Exited,
                status: "Exited (0)".to_string(),
                ..Default::default()
            },
        ]
    }

    #[test]
    fn test_container_list_creation() {
        let containers = create_test_containers();
        let widget = ContainerListWidget::new(containers);

        assert_eq!(widget.len(), 2);
        assert!(!widget.is_empty());
        assert_eq!(
            widget.selected_container_id(),
            Some("abc123def456".to_string())
        );
    }

    #[test]
    fn test_empty_list() {
        let widget = ContainerListWidget::new(vec![]);
        assert!(widget.is_empty());
        assert_eq!(widget.selected_container_id(), None);
    }

    #[test]
    fn test_navigation() {
        let containers = create_test_containers();
        let mut widget = ContainerListWidget::new(containers);

        // First item selected
        assert_eq!(
            widget.selected_container_id(),
            Some("abc123def456".to_string())
        );

        // Next
        widget.next();
        assert_eq!(
            widget.selected_container_id(),
            Some("def789ghi012".to_string())
        );

        // Wrap around
        widget.next();
        assert_eq!(
            widget.selected_container_id(),
            Some("abc123def456".to_string())
        );

        // Previous
        widget.previous();
        assert_eq!(
            widget.selected_container_id(),
            Some("def789ghi012".to_string())
        );
    }

    #[test]
    fn test_update_preserves_selection() {
        let containers = create_test_containers();
        let mut widget = ContainerListWidget::new(containers);

        // Select second item
        widget.next();
        assert_eq!(
            widget.selected_container_id(),
            Some("def789ghi012".to_string())
        );

        // Update with same containers
        let new_containers = create_test_containers();
        widget.update_containers(new_containers);

        // Should still be on second item
        assert_eq!(
            widget.selected_container_id(),
            Some("def789ghi012".to_string())
        );
    }
}

//! Network list widget

use crate::ui::theme;
use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table, TableState},
};

use crate::core::NetworkSummary;

/// Widget for displaying a list of Docker networks
pub struct NetworkListWidget {
    networks: Vec<NetworkSummary>,
    state: TableState,
}

impl NetworkListWidget {
    /// Create a new network list widget
    pub fn new(networks: Vec<NetworkSummary>) -> Self {
        let mut state = TableState::default();
        if !networks.is_empty() {
            state.select(Some(0));
        }
        Self { networks, state }
    }

    /// Update the network list
    pub fn update_networks(&mut self, networks: Vec<NetworkSummary>) {
        let selected_id = self.selected_network_id();

        self.networks = networks;

        if let Some(id) = selected_id {
            if let Some(idx) = self.networks.iter().position(|n| n.id == id) {
                self.state.select(Some(idx));
            } else if !self.networks.is_empty() {
                self.state.select(Some(0));
            }
        } else if !self.networks.is_empty() && self.state.selected().is_none() {
            self.state.select(Some(0));
        }
    }

    /// Get the selected network ID
    pub fn selected_network_id(&self) -> Option<String> {
        self.state
            .selected()
            .and_then(|idx| self.networks.get(idx))
            .map(|n| n.id.clone())
    }

    /// Get the selected network
    pub fn selected_network(&self) -> Option<&NetworkSummary> {
        self.state.selected().and_then(|idx| self.networks.get(idx))
    }

    /// Move selection down
    pub fn next(&mut self) {
        if self.networks.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.networks.len() - 1 {
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
        if self.networks.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.networks.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Get the number of networks
    pub fn len(&self) -> usize {
        self.networks.len()
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.networks.is_empty()
    }

    /// Build the table widget
    pub fn build_table(&self) -> Table<'_> {
        let header = Row::new(vec!["NAME", "DRIVER", "SCOPE", "CONTAINERS"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(0);

        let rows: Vec<Row> = self
            .networks
            .iter()
            .map(|n| {
                // Style for special networks
                let style = if n.name == "bridge" || n.name == "host" || n.name == "none" {
                    theme::accent()
                } else if n.ingress {
                    Style::default().fg(Color::Magenta)
                } else if n.internal {
                    theme::warn()
                } else {
                    Style::default()
                };

                let scope = format!("{:?}", n.scope).to_lowercase();
                let containers = format!("{}", n.connected_containers.len());

                Row::new(vec![
                    Line::from(Span::styled(n.name.clone(), style)),
                    Line::from(n.driver.clone()),
                    Line::from(scope),
                    Line::from(containers),
                ])
            })
            .collect();

        Table::new(
            rows,
            [
                Constraint::Min(20),    // Name
                Constraint::Length(12), // Driver
                Constraint::Length(8),  // Scope
                Constraint::Length(10), // Containers
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(" Networks ({}) ", self.networks.len()))
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
    use crate::core::NetworkScope;
    use chrono::Utc;

    fn create_test_networks() -> Vec<NetworkSummary> {
        vec![
            NetworkSummary {
                id: "abc123".to_string(),
                name: "bridge".to_string(),
                driver: "bridge".to_string(),
                scope: NetworkScope::Local,
                created: Utc::now(),
                internal: false,
                attachable: false,
                ingress: false,
                enable_ipv6: false,
                connected_containers: vec!["c1".to_string(), "c2".to_string()],
            },
            NetworkSummary {
                id: "def456".to_string(),
                name: "my-network".to_string(),
                driver: "bridge".to_string(),
                scope: NetworkScope::Local,
                created: Utc::now(),
                internal: false,
                attachable: true,
                ingress: false,
                enable_ipv6: false,
                connected_containers: vec![],
            },
        ]
    }

    #[test]
    fn test_network_list_creation() {
        let networks = create_test_networks();
        let widget = NetworkListWidget::new(networks);
        assert_eq!(widget.len(), 2);
        assert!(!widget.is_empty());
    }

    #[test]
    fn test_navigation() {
        let networks = create_test_networks();
        let mut widget = NetworkListWidget::new(networks);

        assert_eq!(widget.state.selected(), Some(0));

        widget.next();
        assert_eq!(widget.state.selected(), Some(1));

        widget.next();
        assert_eq!(widget.state.selected(), Some(0)); // Wrap around
    }
}

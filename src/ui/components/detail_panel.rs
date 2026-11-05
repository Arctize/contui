//! Container detail panel widget

use crate::ui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::core::{ContainerState, ContainerSummary, MountPoint, PortMapping};

/// Widget for displaying container details
pub struct ContainerDetailPanel;

impl ContainerDetailPanel {
    /// Draw the detail panel for a container
    pub fn draw(container: &ContainerSummary) -> Paragraph<'static> {
        let text = Self::format_container_info(container);

        Paragraph::new(text)
            .block(
                Block::default()
                    .title(format!(
                        " {} ",
                        container
                            .names
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "Container".to_string())
                    ))
                    .borders(Borders::ALL)
                    .border_style(theme::subtle())
                    .style(Style::default().bg(theme::bg())),
            )
            .wrap(Wrap { trim: true })
    }

    /// Format container information as text
    fn format_container_info(container: &ContainerSummary) -> Vec<Line<'static>> {
        let mut lines = vec![];

        // Status line with color
        let status_color = match container.state {
            ContainerState::Running => Color::Green,
            ContainerState::Paused => Color::Yellow,
            ContainerState::Exited | ContainerState::Dead => Color::Red,
            _ => theme::muted().fg.unwrap_or(Color::Reset),
        };

        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(container.status.clone(), Style::default().fg(status_color)),
        ]));

        lines.push(Line::from(""));

        // Basic info
        lines.push(Self::info_line_owned("ID:", container.id.clone()));
        lines.push(Self::info_line_owned("Image:", container.image.clone()));
        lines.push(Self::info_line_owned("Command:", container.command.clone()));
        lines.push(Self::info_line_owned(
            "Created:",
            container.created.format("%Y-%m-%d %H:%M:%S").to_string(),
        ));

        // Ports
        if !container.ports.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Ports:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for port in &container.ports {
                lines.push(Self::format_port(port));
            }
        }

        // Mounts
        if !container.mounts.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Mounts:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for mount in &container.mounts {
                lines.push(Self::format_mount(mount));
            }
        }

        // Networks
        if !container.networks.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Networks:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(container.networks.join(", ")));
        }

        // Compose info
        if let Some(ref project) = container.compose_project {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Docker Compose:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Self::info_line_owned("Project:", project.clone()));
            if let Some(ref service) = container.compose_service {
                lines.push(Self::info_line_owned("Service:", service.clone()));
            }
        }

        // Labels (if any, show count)
        if !container.labels.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Labels: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{} labels", container.labels.len())),
            ]));
        }

        // Actions hint
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Actions: [s] Start/Stop  [r] Restart  [p] Pause  [k] Kill  [d] Delete  [l] Logs",
            theme::accent(),
        )));

        lines
    }

    /// Create an info line with label and value (owned strings)
    fn info_line_owned(label: &str, value: String) -> Line<'static> {
        Line::from(vec![
            Span::styled(
                label.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::raw(value),
        ])
    }

    /// Format a port mapping
    fn format_port(port: &PortMapping) -> Line<'static> {
        let ip = port.ip.clone().unwrap_or_else(|| "0.0.0.0".to_string());
        let text = if let Some(public) = port.public_port {
            format!(
                "{}:{} → {}:{}",
                ip, public, port.private_port, port.protocol
            )
        } else {
            format!("{}:{}", port.private_port, port.protocol)
        };
        Line::from(format!("  • {}", text))
    }

    /// Format a mount point
    fn format_mount(mount: &MountPoint) -> Line<'static> {
        let mode = if mount.rw { "rw" } else { "ro" };
        Line::from(format!(
            "  • {} → {} ({}, {:?})",
            mount.source, mount.destination, mode, mount.typ
        ))
    }
}

/// Layout helper for split view
pub struct SplitLayout;

impl SplitLayout {
    /// Create a horizontal split layout (list | detail)
    pub fn horizontal_split(area: Rect, list_ratio: u16) -> (Rect, Rect) {
        let constraints = if list_ratio >= 100 {
            vec![Constraint::Min(0), Constraint::Length(0)]
        } else if list_ratio == 0 {
            vec![Constraint::Length(0), Constraint::Min(0)]
        } else {
            vec![
                Constraint::Percentage(list_ratio),
                Constraint::Percentage(100 - list_ratio),
            ]
        };

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        (chunks[0], chunks[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_container() -> ContainerSummary {
        ContainerSummary {
            id: "abc123def456789".to_string(),
            short_id: "abc123".to_string(),
            names: vec!["test-container".to_string()],
            image: "nginx:latest".to_string(),
            image_id: "sha256:123".to_string(),
            command: "nginx -g daemon off;".to_string(),
            created: chrono::Utc::now(),
            ports: vec![PortMapping {
                ip: Some("0.0.0.0".to_string()),
                private_port: 80,
                public_port: Some(8080),
                protocol: "tcp".to_string(),
            }],
            size_rw: Some(1024),
            size_root_fs: Some(1024000),
            labels: std::collections::HashMap::new(),
            state: ContainerState::Running,
            status: "Up 2 hours".to_string(),
            health: None,
            mounts: vec![],
            networks: vec!["bridge".to_string()],
            compose_project: Some("myapp".to_string()),
            compose_service: Some("web".to_string()),
        }
    }

    #[test]
    fn test_format_container_info() {
        let container = create_test_container();
        let lines = ContainerDetailPanel::format_container_info(&container);

        assert!(!lines.is_empty());

        // Check that status line exists
        let status_line = lines.iter().find(|l| l.to_string().contains("Status:"));
        assert!(status_line.is_some());
    }

    #[test]
    fn test_draw() {
        let container = create_test_container();
        let paragraph = ContainerDetailPanel::draw(&container);

        // Just verify it doesn't panic
        let _ = paragraph;
    }

    #[test]
    fn test_format_port() {
        let port = PortMapping {
            ip: Some("127.0.0.1".to_string()),
            private_port: 5432,
            public_port: Some(5433),
            protocol: "tcp".to_string(),
        };

        let line = ContainerDetailPanel::format_port(&port);
        let text = line.to_string();
        assert!(text.contains("127.0.0.1:5433"));
        assert!(text.contains("5432"));
    }

    #[test]
    fn test_split_layout() {
        let area = Rect::new(0, 0, 100, 30);
        let (left, right) = SplitLayout::horizontal_split(area, 50);

        assert_eq!(left.width, 50);
        assert_eq!(right.width, 50);
    }
}

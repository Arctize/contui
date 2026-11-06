//! Compose project and service tables

use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table},
};

use crate::core::compose::{ComposeProject, GroupStatus};

fn status_style(status: GroupStatus) -> Style {
    match status {
        GroupStatus::Running => Style::default().fg(Color::Green),
        GroupStatus::Partial => Style::default().fg(Color::Yellow),
        GroupStatus::Stopped => Style::default().fg(Color::Gray),
    }
}

fn header(columns: Vec<&'static str>) -> Row<'static> {
    Row::new(columns).style(Style::default().add_modifier(Modifier::BOLD))
}

fn highlight() -> Style {
    Style::default()
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

/// All compose projects, one row each
pub fn projects_table(projects: &[ComposeProject], other_containers: usize) -> Table<'static> {
    let rows: Vec<Row> = projects
        .iter()
        .map(|p| {
            Row::new(vec![
                Line::from(p.name.clone()),
                Line::from(p.services.len().to_string()),
                Line::from(format!("{}/{}", p.running, p.total)),
                Line::from(Span::styled(p.status.label(), status_style(p.status))),
                Line::from(p.working_dir.clone().unwrap_or_else(|| "-".to_string())),
            ])
        })
        .collect();

    let mut title = format!(" Compose projects ({}) ", projects.len());
    if other_containers > 0 {
        title = format!(
            " Compose projects ({}) · {} other containers ",
            projects.len(),
            other_containers
        );
    }

    Table::new(
        rows,
        [
            Constraint::Min(16),    // Project
            Constraint::Length(9),  // Services
            Constraint::Length(11), // Containers running/total
            Constraint::Length(9),  // Status
            Constraint::Min(20),    // Working dir
        ],
    )
    .header(header(vec![
        "PROJECT",
        "SERVICES",
        "RUNNING",
        "STATUS",
        "DIRECTORY",
    ]))
    .block(Block::default().title(title).borders(Borders::ALL))
    .row_highlight_style(highlight())
    .highlight_symbol("▶ ")
}

/// The services of one project, one row each
pub fn services_table(project: &ComposeProject) -> Table<'static> {
    let rows: Vec<Row> = project
        .services
        .iter()
        .map(|s| {
            Row::new(vec![
                Line::from(s.name.clone()),
                Line::from(format!("{}/{}", s.running, s.total)),
                Line::from(Span::styled(s.detail.clone(), status_style(s.status))),
                Line::from(s.image.clone()),
            ])
        })
        .collect();

    let title = format!(
        " {} ({}/{} running) · Esc: back ",
        project.name, project.running, project.total
    );

    Table::new(
        rows,
        [
            Constraint::Min(16),   // Service
            Constraint::Length(9), // Replicas
            Constraint::Min(22),   // Status
            Constraint::Min(20),   // Image
        ],
    )
    .header(header(vec!["SERVICE", "RUNNING", "STATUS", "IMAGE"]))
    .block(Block::default().title(title).borders(Borders::ALL))
    .row_highlight_style(highlight())
    .highlight_symbol("▶ ")
}

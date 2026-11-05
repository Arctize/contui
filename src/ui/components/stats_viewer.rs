//! Stats viewer component - bottom panel style

use crate::ui::theme;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::docker::format_bytes;
use crate::state::StatsViewState;

/// Height of the stats panel
pub const STATS_PANEL_HEIGHT: u16 = 8;

/// Render the stats viewer as a bottom panel
pub fn render_stats_panel(frame: &mut Frame, area: Rect, state: &StatsViewState) {
    let block = Block::default()
        .title(format!(
            " {} Stats {} ",
            state.container_name,
            if state.follow { "[LIVE]" } else { "[PAUSED]" }
        ))
        .borders(Borders::ALL)
        .border_style(theme::accent());

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Check for error
    if let Some(ref error) = state.error {
        let error_text = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        frame.render_widget(error_text, inner_area);
        return;
    }

    // Check if we have stats
    let stats = match state.stats {
        Some(ref s) => s,
        None => {
            let loading = Paragraph::new("Loading stats...")
                .style(theme::title())
                .alignment(Alignment::Center);
            frame.render_widget(loading, inner_area);
            return;
        }
    };

    // Create two-column layout
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner_area);

    // Left column: CPU, Memory
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(columns[0]);

    // Right column: Network, Block I/O, PIDs
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(columns[1]);

    let label_style = theme::muted();
    let value_style = theme::text();
    let bar_style = Style::default().fg(Color::Green);
    let capacity_style = theme::accent();

    // CPU Usage
    let cpu_bar = render_bar(stats.cpu_percent, 15);
    let cpu_line = Paragraph::new(Line::from(vec![
        Span::styled("CPU    ", label_style),
        Span::styled(format!("{:>5.1}%", stats.cpu_percent), value_style),
        Span::styled(" ", value_style),
        Span::styled(cpu_bar, bar_style),
    ]));
    frame.render_widget(cpu_line, left_chunks[1]);

    // Memory Usage
    let mem_bar = render_bar(stats.memory_percent, 15);
    let mem_line = Paragraph::new(Line::from(vec![
        Span::styled("Memory ", label_style),
        Span::styled(format!("{:>5.1}%", stats.memory_percent), value_style),
        Span::styled(" ", value_style),
        Span::styled(mem_bar, bar_style),
        Span::styled(
            format!(
                "  {} / {}",
                format_bytes(stats.memory_usage),
                format_bytes(stats.memory_limit)
            ),
            capacity_style,
        ),
    ]));
    frame.render_widget(mem_line, right_chunks[1]);

    // Network I/O
    let net_line = Paragraph::new(Line::from(vec![
        Span::styled("Net I/O  ", label_style),
        Span::styled(
            format!(
                "↓ {}   ↑ {}",
                format_bytes(stats.network_rx),
                format_bytes(stats.network_tx)
            ),
            value_style,
        ),
    ]));
    frame.render_widget(net_line, left_chunks[2]);

    // Block I/O + PIDs on same line in right column
    let block_pids_line = Paragraph::new(Line::from(vec![
        Span::styled("Blk I/O  ", label_style),
        Span::styled(
            format!(
                "R {}  W {}  |  PIDs: {}",
                format_bytes(stats.block_read),
                format_bytes(stats.block_write),
                stats.pids
            ),
            value_style,
        ),
    ]));
    frame.render_widget(block_pids_line, right_chunks[2]);
}

/// Render a simple ASCII progress bar
fn render_bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64).clamp(0.0, width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

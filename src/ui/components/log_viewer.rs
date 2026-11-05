//! Log viewer widget

use crate::ui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::state::{LogLevelFilter, LogViewState};

/// Detect log level from message content
fn detect_log_level(message: &str) -> LogLevelFilter {
    let msg_upper = message.to_uppercase();
    if msg_upper.contains("ERROR") || msg_upper.contains("FATAL") || msg_upper.contains("ERR:") {
        LogLevelFilter::Error
    } else if msg_upper.contains("WARN") || msg_upper.contains("WARNING") {
        LogLevelFilter::Warn
    } else if msg_upper.contains("INFO")
        || msg_upper.contains("DEBUG")
        || msg_upper.contains("TRACE")
    {
        LogLevelFilter::Info
    } else {
        LogLevelFilter::All
    }
}

/// Render the log viewer overlay
pub fn render_log_viewer(frame: &mut Frame, area: Rect, state: &LogViewState) {
    // Use 90% of screen for log viewer
    let popup_area = centered_rect(90, 90, area);

    // Clear background
    frame.render_widget(Clear, popup_area);

    // Split area for logs and optional search bar
    let (content_area, search_area) = if state.show_search_input {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(popup_area);
        (layout[0], Some(layout[1]))
    } else {
        (popup_area, None)
    };

    // Build title with follow, filter, time, and search indicators
    let filter_indicator = match state.level_filter {
        LogLevelFilter::Error => " [ERROR]",
        LogLevelFilter::Warn => " [WARN]",
        LogLevelFilter::Info => " [INFO]",
        LogLevelFilter::All => "",
    };
    let time_indicator = state
        .time_filter
        .map(|t| {
            let elapsed = chrono::Utc::now() - t;
            if elapsed.num_hours() > 0 {
                format!(" [{}h]", elapsed.num_hours())
            } else if elapsed.num_minutes() > 0 {
                format!(" [{}m]", elapsed.num_minutes())
            } else {
                " [<1m]".to_string()
            }
        })
        .unwrap_or_default();
    let search_indicator = if state.search_pattern.is_some() {
        format!(" [SEARCH: {}]", state.search_pattern.as_ref().unwrap())
    } else {
        String::new()
    };
    let title = format!(
        " Logs: {} {}{}{}{} ",
        state.container_name,
        if state.follow { "[FOLLOW]" } else { "" },
        filter_indicator,
        time_indicator,
        search_indicator
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::accent())
        .style(Style::default().bg(theme::bg()));

    let inner_area = block.inner(content_area);
    frame.render_widget(block, content_area);

    // CRITICAL: Fill inner area with black background first to prevent ghost text
    let bg_fill = Block::default().style(Style::default().bg(theme::bg()));
    frame.render_widget(bg_fill, inner_area);

    // Render search input if active
    if let Some(search_area) = search_area {
        let match_text = if state.search_matches.is_empty() {
            "0/0".to_string()
        } else {
            format!(
                "{}/{}",
                state.current_match.map(|i| i + 1).unwrap_or(0),
                state.search_matches.len()
            )
        };
        let search_text = format!(
            "/{}  {}",
            state.search_pattern.as_deref().unwrap_or(""),
            match_text
        );
        let search_block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::title())
            .style(Style::default().bg(theme::bg()));
        let search_para = Paragraph::new(search_text)
            .style(theme::title().bg(theme::bg()))
            .block(search_block);
        frame.render_widget(search_para, search_area);
    }

    // Filter logs based on level filter and time filter
    let filtered_logs: Vec<(usize, &crate::docker::LogEntry)> = state
        .logs
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            // Level filter
            let level_match = match state.level_filter {
                LogLevelFilter::All => true,
                LogLevelFilter::Error => detect_log_level(&entry.message) == LogLevelFilter::Error,
                LogLevelFilter::Warn => detect_log_level(&entry.message) == LogLevelFilter::Warn,
                LogLevelFilter::Info => detect_log_level(&entry.message) == LogLevelFilter::Info,
            };

            // Time filter (show logs after the cutoff time)
            let time_match = state.time_filter.map_or(true, |cutoff| {
                entry.timestamp.is_some_and(|ts| ts >= cutoff)
            });

            level_match && time_match
        })
        .collect();

    let visible_lines = inner_area.height as usize;
    let total_lines = filtered_logs.len();

    // Show message if no logs
    if state.logs.is_empty() {
        let no_logs = Paragraph::new("Press 'r' to load logs | 'q' to close | 'f' to toggle follow | '/' to search | 0-3 filter")
            .style(theme::title().bg(theme::bg()))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(no_logs, inner_area);
        return;
    }

    if total_lines == 0 {
        let no_filtered_logs = Paragraph::new("No logs match current filter | Press 0 to show all")
            .style(theme::title().bg(theme::bg()))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(no_filtered_logs, inner_area);
        return;
    }

    // Calculate start index based on scroll offset
    let start_idx = if total_lines > visible_lines {
        state.scroll_offset.saturating_sub(visible_lines / 2)
    } else {
        0
    };
    let start_idx = start_idx.min(total_lines.saturating_sub(visible_lines));

    // Build visible log lines with search highlighting
    let end_idx = (start_idx + visible_lines).min(total_lines);
    let log_lines: Vec<Line> = (start_idx..end_idx)
        .map(|idx| {
            let (original_idx, entry) = filtered_logs[idx];
            let timestamp = entry
                .timestamp
                .map(|ts: chrono::DateTime<chrono::Utc>| ts.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "??:??:??".to_string());

            // Check if this line is a search match
            let is_match = state.search_matches.contains(&original_idx);
            let is_current_match = state
                .current_match
                .map(|current_idx| state.search_matches.get(current_idx) == Some(&original_idx))
                .unwrap_or(false);

            let mut style = if entry.is_stderr {
                Style::default().fg(Color::Red)
            } else {
                theme::text()
            };

            // Highlight search matches
            if is_current_match {
                style = style.patch(theme::search_match(true));
            } else if is_match {
                style = style.patch(theme::search_match(false));
            }

            Line::from(vec![
                Span::styled(format!("{} ", timestamp), theme::subtle()),
                Span::styled(entry.message.clone(), style),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(log_lines)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(theme::bg()));

    frame.render_widget(paragraph, inner_area);

    // Render scrollbar indicator if needed
    if total_lines > visible_lines {
        let scroll_pct = (state.scroll_offset as f64 / (total_lines - 1) as f64 * 100.0) as u16;
        let scroll_indicator = format!("{}%", scroll_pct);
        let scroll_area = Rect::new(popup_area.right().saturating_sub(7), popup_area.y + 1, 5, 1);
        // Ensure scroll indicator doesn't overlap border
        if scroll_area.x > popup_area.x + 1 {
            frame.render_widget(
                Paragraph::new(scroll_indicator).style(theme::subtle().bg(theme::bg())),
                scroll_area,
            );
        }
    }
}

/// Calculate centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
            ratatui::layout::Constraint::Percentage(percent_y),
            ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
            ratatui::layout::Constraint::Percentage(percent_x),
            ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::LogEntry;

    #[test]
    fn test_log_viewer_creation() {
        use crate::state::LogLevelFilter;

        let state = LogViewState {
            container_id: "abc123".to_string(),
            container_name: "test-container".to_string(),
            logs: vec![LogEntry {
                timestamp: None,
                message: "Test log line".to_string(),
                is_stderr: false,
            }],
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
        };

        assert_eq!(state.container_name, "test-container");
        assert!(state.follow);
    }
}

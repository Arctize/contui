//! Container detail viewer component

use crate::ui::theme;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::state::DetailViewState;

/// Render the detail viewer overlay
pub fn render_detail_viewer(frame: &mut Frame, area: Rect, state: &DetailViewState) {
    // Use 80% of screen for detail viewer
    let popup_area = centered_rect(80, 85, area);

    // Clear background first
    frame.render_widget(Clear, popup_area);

    // Build title
    let title = format!(" Container: {} ", state.container_name);

    // Create block with explicit background for the popup border + area
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::accent().bg(theme::bg()))
        .style(Style::default().bg(theme::bg()));

    let inner_area = block.inner(popup_area);

    // Check if we have details
    let details = match state.details {
        Some(ref d) => d,
        None => {
            let loading = Paragraph::new("Loading details...")
                .style(theme::title().bg(theme::bg()))
                .alignment(Alignment::Center)
                .block(block);
            frame.render_widget(loading, popup_area);
            return;
        }
    };

    // Build content lines
    let mut lines = vec![];

    let label_style = theme::muted().bg(theme::bg());
    let value_style = theme::text().bg(theme::bg());
    let accent_style = theme::accent().bg(theme::bg());

    // Basic Info
    lines.push(Line::from(vec![
        Span::styled("ID:         ", label_style),
        Span::styled(&details.id, value_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Name:       ", label_style),
        Span::styled(&details.name, value_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Image:      ", label_style),
        Span::styled(&details.image, value_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Status:     ", label_style),
        Span::styled(&details.status, accent_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Created:    ", label_style),
        Span::styled(&details.created, value_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Restart:    ", label_style),
        Span::styled(&details.restart_policy, value_style),
    ]));
    lines.push(Line::from(""));

    // Command
    if let Some(ref cmd) = details.command {
        lines.push(Line::from(vec![
            Span::styled("Command:    ", label_style),
            Span::styled(cmd.as_str(), value_style),
        ]));
    }
    if let Some(ref entry) = details.entrypoint {
        lines.push(Line::from(vec![
            Span::styled("Entrypoint: ", label_style),
            Span::styled(format!("[{}]", entry.join(", ")), value_style),
        ]));
    }
    if details.command.is_some() || details.entrypoint.is_some() {
        lines.push(Line::from(""));
    }

    // Ports
    if !details.ports.is_empty() {
        lines.push(Line::from(vec![Span::styled("Ports:", label_style)]));
        for port in &details.ports {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}:{}/{}", port.host_ip, port.host_port, port.protocol),
                    accent_style,
                ),
                Span::styled(format!(" → {}", port.port), value_style),
            ]));
        }
        lines.push(Line::from(""));
    }

    // Mounts
    if !details.mounts.is_empty() {
        lines.push(Line::from(vec![Span::styled("Mounts:", label_style)]));
        for mount in &details.mounts {
            lines.push(Line::from(vec![
                Span::styled(format!("  {}", mount.source), value_style),
                Span::styled(" → ", label_style),
                Span::styled(
                    format!("{} ({})", mount.destination, mount.mode),
                    accent_style,
                ),
            ]));
        }
        lines.push(Line::from(""));
    }

    // Environment
    if !details.env.is_empty() {
        lines.push(Line::from(vec![Span::styled("Environment:", label_style)]));
        for env in &details.env {
            // Split on first '=' to get key=value
            let parts: Vec<&str> = env.splitn(2, '=').collect();
            if parts.len() == 2 {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}=", parts[0]), accent_style),
                    Span::styled(parts[1], value_style),
                ]));
            }
        }
        lines.push(Line::from(""));
    }

    // Labels
    if !details.labels.is_empty() {
        lines.push(Line::from(vec![Span::styled("Labels:", label_style)]));
        for (key, value) in &details.labels {
            lines.push(Line::from(vec![
                Span::styled(format!("  {}=", key), accent_style),
                Span::styled(value.as_str(), value_style),
            ]));
        }
        lines.push(Line::from(""));
    }

    // Networks
    if !details.networks.is_empty() {
        lines.push(Line::from(vec![Span::styled("Networks:", label_style)]));
        for network in &details.networks {
            lines.push(Line::from(vec![
                Span::styled(format!("  {}  ", network.name), accent_style),
                Span::styled(format!("IP: {}", network.ip_address), value_style),
            ]));
            lines.push(Line::from(vec![
                Span::styled("     ", label_style),
                Span::styled(
                    format!("Gateway: {}  MAC: {}", network.gateway, network.mac_address),
                    value_style,
                ),
            ]));
        }
    }

    let wrapped_lines = wrap_lines_to_width(&lines, inner_area.width);

    // Calculate actual content height and clamp scroll offset (use wrapped height).
    let content_height = wrapped_lines.len();
    let max_scroll = content_height.saturating_sub(inner_area.height as usize);
    let scroll = state.scroll_offset.min(max_scroll);

    let paragraph = Paragraph::new(wrapped_lines)
        .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0))
        .style(Style::default().bg(theme::bg()))
        .block(block);

    frame.render_widget(paragraph, popup_area);
}

fn wrap_lines_to_width(lines: &[Line], width: u16) -> Vec<Line<'static>> {
    let width = width as usize;
    if width == 0 {
        return vec![];
    }

    let mut out = Vec::new();
    for line in lines {
        // Preserve blank lines
        if line.spans.is_empty() || line.spans.iter().all(|s| s.content.is_empty()) {
            out.push(Line::from(""));
            continue;
        }

        // Detect a prefix span used for alignment (e.g. "ID:     " or "  10KB  ").
        let (prefix_span, content_spans) =
            if line.spans.len() >= 2 && line.spans[0].content.ends_with(' ') {
                (Some(&line.spans[0]), &line.spans[1..])
            } else {
                (None, &line.spans[..])
            };

        let mut prefix_text = String::new();
        let mut prefix_style = Style::default();
        if let Some(prefix_span) = prefix_span {
            prefix_text.push_str(prefix_span.content.as_ref());
            prefix_style = prefix_span.style;
        }

        let prefix_width = prefix_text.width();
        let indent_width = if prefix_width < width {
            prefix_width
        } else {
            0
        };
        let indent_span = if indent_width > 0 {
            Some(Span::styled(" ".repeat(indent_width), prefix_style))
        } else {
            None
        };

        let mut current_spans: Vec<Span<'static>> = Vec::new();
        let mut current_width = 0usize;

        if !prefix_text.is_empty() && prefix_width < width {
            current_spans.push(Span::styled(prefix_text, prefix_style));
            current_width = prefix_width;
        }

        let flush = |spans: &mut Vec<Span<'static>>, out: &mut Vec<Line<'static>>| {
            if spans.is_empty() {
                out.push(Line::from(""));
            } else {
                out.push(Line::from(std::mem::take(spans)));
            }
        };

        for span in content_spans {
            let style = span.style;
            let mut chunk = String::new();
            let mut chunk_width = 0usize;

            for ch in span.content.chars() {
                let ch_width = ch.width().unwrap_or(0);
                if ch_width == 0 {
                    continue;
                }

                if current_width + chunk_width + ch_width > width {
                    if !chunk.is_empty() {
                        current_spans.push(Span::styled(std::mem::take(&mut chunk), style));
                        chunk_width = 0;
                    }

                    flush(&mut current_spans, &mut out);

                    if let Some(ref indent_span) = indent_span {
                        current_spans.push(indent_span.clone());
                        current_width = indent_width;
                    } else {
                        current_width = 0;
                    }

                    // Avoid leading spaces on wrapped lines
                    if ch == ' ' && current_width == indent_width {
                        continue;
                    }
                }

                chunk.push(ch);
                chunk_width += ch_width;
            }

            if !chunk.is_empty() {
                current_spans.push(Span::styled(chunk, style));
                current_width += chunk_width;
            }
        }

        flush(&mut current_spans, &mut out);
    }

    out
}

/// Calculate centered rectangle for popup
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let width = ((r.width as u32) * (percent_x as u32) / 100) as u16;
    let height = ((r.height as u32) * (percent_y as u32) / 100) as u16;
    let width = width.clamp(3, r.width);
    let height = height.clamp(3, r.height);
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_lines_wraps_and_indents() {
        let label = Span::styled("Key:  ", theme::muted());
        let value = Span::styled("abcdefghijklmnopqrstuvwxyz", theme::text());
        let lines = vec![Line::from(vec![label, value])];
        let wrapped = wrap_lines_to_width(&lines, 10);

        assert!(wrapped.len() > 1);

        // Continuation lines start with the same indentation width as the label prefix.
        let indent_width = "Key:  ".width();
        let first = wrapped[1].spans.first().unwrap().content.as_ref();
        assert_eq!(first.width(), indent_width);
        assert!(first.chars().all(|c| c == ' '));

        for line in wrapped {
            let line_width: usize = line.spans.iter().map(|s| s.content.width()).sum();
            assert!(line_width <= 10);
        }
    }

    #[test]
    fn wrap_lines_preserves_blank_lines() {
        let lines = vec![Line::from(""), Line::from("abc")];
        let wrapped = wrap_lines_to_width(&lines, 5);
        assert_eq!(wrapped.len(), 2);
        assert!(
            wrapped[0].spans.is_empty() || wrapped[0].spans.iter().all(|s| s.content.is_empty())
        );
    }
}

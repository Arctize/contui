//! Image detail viewer component

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::view_search;
use crate::docker::{format_signed_size, format_size};
use crate::state::ImageDetailViewState;

/// Render the image detail viewer overlay
pub fn render_image_detail_viewer(frame: &mut Frame, area: Rect, state: &ImageDetailViewState) {
    // Use 80% of screen for detail viewer
    let popup_area = centered_rect(80, 85, area);

    // Clear background first
    frame.render_widget(Clear, popup_area);

    // Build title
    let title = format!(" Image: {} ", state.image_name);

    // Create block with explicit background for the popup border + area
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .style(Style::default().bg(Color::Black));

    let inner_area = block.inner(popup_area);

    // Check if we have details
    let details = match state.details {
        Some(ref d) => d,
        None => {
            let loading = Paragraph::new("Loading image details...")
                .style(Style::default().fg(Color::Yellow).bg(Color::Black))
                .alignment(Alignment::Center)
                .block(block);
            frame.render_widget(loading, popup_area);
            return;
        }
    };

    let lines = build_lines(details);
    let searched = view_search::layout(
        &lines,
        inner_area.width,
        &state.search.query,
        state.search.current,
        wrap_lines_to_width,
    );
    let footer = view_search::footer(&state.search, searched.match_starts.len());

    // The search status line takes the last row of the popup
    let (content_area, footer_area) = match footer {
        Some(_) if inner_area.height > 1 => (
            Rect::new(
                inner_area.x,
                inner_area.y,
                inner_area.width,
                inner_area.height - 1,
            ),
            Some(Rect::new(
                inner_area.x,
                inner_area.y + inner_area.height - 1,
                inner_area.width,
                1,
            )),
        ),
        _ => (inner_area, None),
    };

    // Calculate actual content height and clamp scroll offset (use wrapped height).
    let wrapped_lines = searched.lines;
    let content_height = wrapped_lines.len();
    let max_scroll = content_height.saturating_sub(content_area.height as usize);
    let scroll = state.scroll_offset.min(max_scroll);

    frame.render_widget(block, popup_area);

    let paragraph = Paragraph::new(wrapped_lines)
        .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0))
        .style(Style::default().bg(Color::Black));
    frame.render_widget(paragraph, content_area);

    if let (Some(line), Some(area)) = (footer, footer_area) {
        frame.render_widget(
            Paragraph::new(line).style(Style::default().fg(Color::Yellow).bg(Color::Black)),
            area,
        );
    }
}

/// Build the (unwrapped) content lines
fn build_lines(details: &crate::docker::ImageDetails) -> Vec<Line<'_>> {
    let mut lines = vec![];

    let label_style = Style::default().fg(Color::Gray).bg(Color::Black);
    let value_style = Style::default().fg(Color::White).bg(Color::Black);
    let accent_style = Style::default().fg(Color::Cyan).bg(Color::Black);

    // Basic Info
    lines.push(Line::from(vec![
        Span::styled("ID:         ", label_style),
        Span::styled(&details.id, value_style),
    ]));

    // Tags
    if !details.repo_tags.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Tags:       ", label_style),
            Span::styled(details.repo_tags.join(", "), value_style),
        ]));
    }

    // Size
    lines.push(Line::from(vec![
        Span::styled("Size:       ", label_style),
        Span::styled(format_size(details.size), accent_style),
    ]));

    // Created
    lines.push(Line::from(vec![
        Span::styled("Created:    ", label_style),
        Span::styled(&details.created, value_style),
    ]));

    // Author
    if !details.author.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Author:     ", label_style),
            Span::styled(&details.author, value_style),
        ]));
    }

    // OS/Arch
    lines.push(Line::from(vec![
        Span::styled("OS/Arch:    ", label_style),
        Span::styled(
            format!("{}/{}", details.os, details.architecture),
            value_style,
        ),
    ]));
    lines.push(Line::from(""));

    // Exposed Ports
    if !details.exposed_ports.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "Exposed Ports:",
            label_style,
        )]));
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", details.exposed_ports.join(", ")),
            value_style,
        )]));
        lines.push(Line::from(""));
    }

    // Environment
    if !details.env.is_empty() {
        lines.push(Line::from(vec![Span::styled("Environment:", label_style)]));
        for env in &details.env {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", env),
                value_style,
            )]));
        }
        lines.push(Line::from(""));
    }

    // Entrypoint
    if let Some(ref entry) = details.entrypoint {
        lines.push(Line::from(vec![
            Span::styled("Entrypoint: ", label_style),
            Span::styled(format!("[{}]", entry.join(", ")), value_style),
        ]));
    }

    // Command
    if let Some(ref cmd) = details.cmd {
        lines.push(Line::from(vec![
            Span::styled("Command:    ", label_style),
            Span::styled(format!("[{}]", cmd.join(", ")), value_style),
        ]));
    }
    if details.entrypoint.is_some() || details.cmd.is_some() {
        lines.push(Line::from(""));
    }

    // Layers
    if !details.layers.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            format!("Layers ({}):", details.layers.len()),
            label_style,
        )]));
        for layer in &details.layers {
            let size_str = format_signed_size(layer.size);
            let cmd = if layer.created_by.is_empty() {
                "missing".to_string()
            } else {
                layer.created_by.clone()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:>10}  ", size_str), accent_style),
                Span::styled(cmd, value_style),
            ]));
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
    }

    lines
}

/// Wrapped start line of every search match for `query`, for jumping with n/N.
/// `area` is the full screen area the viewer is rendered into.
pub fn match_offsets(details: &crate::docker::ImageDetails, query: &str, area: Rect) -> Vec<usize> {
    let popup_area = centered_rect(80, 85, area);
    let inner = Block::default().borders(Borders::ALL).inner(popup_area);
    let lines = build_lines(details);
    view_search::layout(&lines, inner.width, query, 0, wrap_lines_to_width).match_starts
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
        let label = Span::styled("Key:  ", Style::default().fg(Color::Gray));
        let value = Span::styled(
            "abcdefghijklmnopqrstuvwxyz",
            Style::default().fg(Color::White),
        );
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
}

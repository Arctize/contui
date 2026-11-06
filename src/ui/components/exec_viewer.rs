//! Exec viewer component - bottom panel style

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::state::ExecViewState;

/// Height of the exec panel
pub const EXEC_PANEL_HEIGHT: u16 = 10;

/// Render the exec viewer as a bottom panel
pub fn render_exec_panel(frame: &mut Frame, area: Rect, state: &ExecViewState) {
    let title = format!(
        " {}: {} [{}] {} ",
        if state.attach { "Attach" } else { "Exec" },
        state.container_name,
        if state.focus { "FOCUS" } else { "UI" },
        state.status
    );

    let border_color = if state.focus {
        Color::Magenta
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = if state.screen_lines.is_empty() {
        vec![Line::from(format!("{}...", state.status))]
    } else {
        state
            .screen_lines
            .iter()
            .take(inner_area.height as usize)
            .map(|l: &String| Line::from(l.as_str()))
            .collect()
    };

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner_area);

    if state.focus {
        if let Some((row, col)) = state.cursor {
            if row < inner_area.height && col < inner_area.width {
                frame.set_cursor_position((
                    inner_area.x.saturating_add(col),
                    inner_area.y.saturating_add(row),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Position;
    use ratatui::Terminal;

    #[test]
    fn renders_exec_header() {
        let backend = TestBackend::new(80, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = ExecViewState {
            container_id: "id".into(),
            container_name: "web".into(),
            focus: true,
            status: "Running".into(),
            screen_lines: vec!["hello".into()],
            cursor: None,
            attach: false,
        };

        terminal
            .draw(|f| render_exec_panel(f, f.area(), &state))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let cell = buffer.cell((0, 0)).expect("cell at 0,0");
        assert!(!cell.symbol().is_empty());
    }

    #[test]
    fn renders_exec_placeholder_uses_status() {
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = ExecViewState {
            container_id: "id".into(),
            container_name: "web".into(),
            focus: true,
            status: "Starting |".into(),
            screen_lines: vec![],
            cursor: None,
            attach: false,
        };

        terminal
            .draw(|f| render_exec_panel(f, f.area(), &state))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut found = false;
        for y in 0..buffer.area.height {
            let line: String = (0..buffer.area.width)
                .filter_map(|x| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
                .collect::<Vec<_>>()
                .join("");
            if line.contains("Starting |...") {
                found = true;
                break;
            }
        }
        assert!(found, "expected placeholder to include status text");
    }

    #[test]
    fn renders_exec_cursor_when_focused() {
        let backend = TestBackend::new(20, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = ExecViewState {
            container_id: "id".into(),
            container_name: "web".into(),
            focus: true,
            status: "Running".into(),
            screen_lines: vec!["hello".into()],
            cursor: Some((1, 2)),
            attach: false,
        };

        terminal
            .draw(|f| render_exec_panel(f, f.area(), &state))
            .unwrap();

        terminal
            .backend_mut()
            .assert_cursor_position(Position::new(3, 2));
    }
}

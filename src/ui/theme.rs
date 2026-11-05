//! Semantic styles for the UI.
//!
//! The default `auto` theme never sets a background and only uses the
//! terminal's own foreground colour plus bold/dim/reverse, so it stays
//! readable on light and dark terminals alike. Status colours (green, red,
//! yellow) are kept because they carry meaning. The `classic` theme keeps the
//! previous fixed palette (black background, white/gray text).

use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::style::{Color, Modifier, Style};

static CLASSIC: AtomicBool = AtomicBool::new(false);

/// Select the theme by its config name. `classic` and the legacy `dark`
/// value keep the old fixed palette; anything else (`auto`, ...) follows the
/// terminal colours.
pub fn set_from_name(name: &str) {
    let classic = matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "classic" | "dark"
    );
    CLASSIC.store(classic, Ordering::Relaxed);
}

fn classic() -> bool {
    CLASSIC.load(Ordering::Relaxed)
}

/// Panel background: the terminal's own (`auto`) or black (`classic`).
pub fn bg() -> Color {
    if classic() {
        Color::Black
    } else {
        Color::Reset
    }
}

/// Normal text.
pub fn text() -> Style {
    if classic() {
        Style::default().fg(Color::White)
    } else {
        Style::default()
    }
}

/// Secondary text such as labels and hints.
pub fn muted() -> Style {
    if classic() {
        Style::default().fg(Color::Gray)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

/// Borders, separators and line numbers.
pub fn subtle() -> Style {
    if classic() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

/// Emphasised labels and focused borders.
pub fn accent() -> Style {
    if classic() {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

/// Dialog titles and borders, section headings.
pub fn title() -> Style {
    if classic() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

/// Something that deserves attention but is not an error (paused, unused).
pub fn warn() -> Style {
    if classic() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }
}

/// The selected row in a table.
pub fn selected() -> Style {
    if classic() {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    }
}

/// A search match in the log viewer; `current` is the focused match.
pub fn search_match(current: bool) -> Style {
    let base = if current {
        Color::Magenta
    } else {
        Color::Yellow
    };
    if classic() {
        Style::default().bg(base).fg(Color::Black)
    } else {
        Style::default()
            .fg(base)
            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_theme_sets_no_background_or_fixed_text_colour() {
        set_from_name("auto");
        assert_eq!(bg(), Color::Reset);
        for s in [text(), muted(), subtle(), accent(), title(), selected()] {
            assert_eq!(s.fg, None);
            assert_eq!(s.bg, None);
        }
    }

    #[test]
    fn classic_theme_keeps_old_palette() {
        for name in ["classic", "dark", "Classic"] {
            set_from_name(name);
            assert_eq!(bg(), Color::Black);
            assert_eq!(muted().fg, Some(Color::Gray));
            assert_eq!(selected().bg, Some(Color::DarkGray));
        }
        set_from_name("auto");
    }
}

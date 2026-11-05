//! Incremental search for the detail viewers.
//!
//! `/` starts typing a query, `Enter` keeps it, `n`/`N` jump to the next or
//! previous match and `Esc` clears it. Matching is a case-insensitive substring
//! search per source line. Matches are located on the *wrapped* lines, because
//! that is what the scroll offset refers to.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::state::ViewSearch;

/// Wrapped content plus where the matches start (wrapped line index).
pub struct SearchLayout {
    pub lines: Vec<Line<'static>>,
    pub match_starts: Vec<usize>,
}

pub type WrapFn = fn(&[Line], u16) -> Vec<Line<'static>>;

fn plain_text(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Wrap `lines` to `width` and mark the lines matching `query`: matches are
/// underlined and the current one (`current` modulo the number of matches) is
/// reversed.
pub fn layout(
    lines: &[Line],
    width: u16,
    query: &str,
    current: usize,
    wrap: WrapFn,
) -> SearchLayout {
    let needle = query.to_lowercase();
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    for line in lines {
        let wrapped = wrap(std::slice::from_ref(line), width);
        if !needle.is_empty() && plain_text(line).to_lowercase().contains(&needle) {
            ranges.push((out.len(), wrapped.len()));
        }
        out.extend(wrapped);
    }

    if !ranges.is_empty() {
        let current = current % ranges.len();
        for (i, &(start, len)) in ranges.iter().enumerate() {
            let modifier = if i == current {
                Modifier::REVERSED
            } else {
                Modifier::UNDERLINED
            };
            for line in &mut out[start..start + len] {
                for span in &mut line.spans {
                    span.style = span.style.add_modifier(modifier);
                }
            }
        }
    }

    SearchLayout {
        lines: out,
        match_starts: ranges.into_iter().map(|(start, _)| start).collect(),
    }
}

/// The status line shown below the content while a search is active.
pub fn footer(search: &ViewSearch, matches: usize) -> Option<Line<'static>> {
    if search.input {
        return Some(Line::from(vec![
            Span::raw(format!("/{}", search.query)),
            Span::styled("█", ratatui::style::Style::default()),
            Span::raw(format!("  ({matches} matches)  Enter: keep  Esc: clear")),
        ]));
    }
    if search.query.is_empty() {
        return None;
    }
    let position = if matches == 0 {
        "no matches".to_string()
    } else {
        format!("{}/{}", search.current % matches + 1, matches)
    };
    Some(Line::from(format!(
        "/{}  {}  n/N: next/prev  Esc: clear",
        search.query, position
    )))
}

/// Handle a key for the search. `offsets` returns the wrapped start line of
/// every match for a query; the scroll offset is moved to the current match.
/// Returns true when the key was consumed.
pub fn handle_key(
    search: &mut ViewSearch,
    scroll: &mut usize,
    key: KeyEvent,
    offsets: impl Fn(&str) -> Vec<usize>,
) -> bool {
    let mut jump = |search: &mut ViewSearch| {
        let starts = offsets(&search.query);
        if !starts.is_empty() {
            let at = starts[search.current % starts.len()];
            *scroll = at.saturating_sub(2);
        }
    };

    if search.input {
        match key.code {
            KeyCode::Esc => *search = ViewSearch::default(),
            KeyCode::Enter => search.input = false,
            KeyCode::Backspace => {
                search.query.pop();
                search.current = 0;
                jump(search);
            }
            KeyCode::Char(c) => {
                search.query.push(c);
                search.current = 0;
                jump(search);
            }
            _ => {}
        }
        return true;
    }

    match key.code {
        KeyCode::Char('/') => {
            *search = ViewSearch {
                input: true,
                ..ViewSearch::default()
            };
            true
        }
        KeyCode::Char('n') if !search.query.is_empty() => {
            search.current = search.current.wrapping_add(1);
            jump(search);
            true
        }
        KeyCode::Char('N') if !search.query.is_empty() => {
            let count = offsets(&search.query).len();
            if count > 0 {
                search.current = (search.current % count + count - 1) % count;
                jump(search);
            }
            true
        }
        KeyCode::Esc if !search.query.is_empty() => {
            *search = ViewSearch::default();
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn no_wrap(lines: &[Line], _width: u16) -> Vec<Line<'static>> {
        lines.iter().map(|l| Line::from(plain_text(l))).collect()
    }

    fn sample() -> Vec<Line<'static>> {
        ["alpha", "Beta", "gamma", "beta two", "delta"]
            .into_iter()
            .map(Line::from)
            .collect()
    }

    #[test]
    fn layout_finds_case_insensitive_matches() {
        let l = layout(&sample(), 80, "BETA", 0, no_wrap);
        assert_eq!(l.match_starts, vec![1, 3]);
        assert!(l.lines[1].spans[0]
            .style
            .add_modifier
            .contains(Modifier::REVERSED));
        assert!(l.lines[3].spans[0]
            .style
            .add_modifier
            .contains(Modifier::UNDERLINED));
        assert_eq!(layout(&sample(), 80, "", 0, no_wrap).match_starts.len(), 0);
    }

    #[test]
    fn keys_search_and_cycle() {
        let lines = sample();
        let offsets = |q: &str| layout(&lines, 80, q, 0, no_wrap).match_starts;
        let mut s = ViewSearch::default();
        let mut scroll = 0usize;

        assert!(!handle_key(
            &mut s,
            &mut scroll,
            key(KeyCode::Char('n')),
            offsets
        ));
        assert!(handle_key(
            &mut s,
            &mut scroll,
            key(KeyCode::Char('/')),
            offsets
        ));
        for c in "beta".chars() {
            handle_key(&mut s, &mut scroll, key(KeyCode::Char(c)), offsets);
        }
        assert_eq!(s.query, "beta");
        handle_key(&mut s, &mut scroll, key(KeyCode::Enter), offsets);
        assert!(!s.input);

        handle_key(&mut s, &mut scroll, key(KeyCode::Char('n')), offsets);
        assert_eq!(s.current % 2, 1);
        assert_eq!(scroll, 1); // match at line 3, minus 2 lines of context
        handle_key(&mut s, &mut scroll, key(KeyCode::Char('n')), offsets);
        assert_eq!(s.current % 2, 0);
        handle_key(&mut s, &mut scroll, key(KeyCode::Char('N')), offsets);
        assert_eq!(s.current % 2, 1);

        assert!(handle_key(&mut s, &mut scroll, key(KeyCode::Esc), offsets));
        assert!(s.query.is_empty());
        // Esc with no search falls through (closes the viewer)
        assert!(!handle_key(&mut s, &mut scroll, key(KeyCode::Esc), offsets));
    }

    #[test]
    fn footer_reports_position() {
        let mut s = ViewSearch {
            query: "x".into(),
            current: 1,
            ..Default::default()
        };
        let text = plain_text(&footer(&s, 3).unwrap());
        assert!(text.contains("2/3"));
        s.query.clear();
        assert!(footer(&s, 0).is_none());
    }
}

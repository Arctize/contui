//! Image list widget

use crate::ui::theme;
use ratatui::{
    layout::Constraint,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table, TableState},
};

use crate::core::ImageSummary;

/// Widget for displaying a list of Docker images
pub struct ImageListWidget {
    images: Vec<ImageSummary>,
    state: TableState,
}

impl ImageListWidget {
    /// Create a new image list widget
    pub fn new(images: Vec<ImageSummary>) -> Self {
        let mut state = TableState::default();
        if !images.is_empty() {
            state.select(Some(0));
        }
        Self { images, state }
    }

    /// Update the image list
    pub fn update_images(&mut self, images: Vec<ImageSummary>) {
        // Preserve selection if possible
        let selected_id = self.selected_image_id();

        self.images = images;

        // Try to restore selection
        if let Some(id) = selected_id {
            if let Some(idx) = self.images.iter().position(|i| i.id == id) {
                self.state.select(Some(idx));
            } else if !self.images.is_empty() {
                self.state.select(Some(0));
            }
        } else if !self.images.is_empty() && self.state.selected().is_none() {
            self.state.select(Some(0));
        }
    }

    /// Get the selected image ID
    pub fn selected_image_id(&self) -> Option<String> {
        self.state
            .selected()
            .and_then(|idx| self.images.get(idx))
            .map(|i| i.id.clone())
    }

    /// Get the selected image
    pub fn selected_image(&self) -> Option<&ImageSummary> {
        self.state.selected().and_then(|idx| self.images.get(idx))
    }

    /// Move selection down
    pub fn next(&mut self) {
        if self.images.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.images.len() - 1 {
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
        if self.images.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.images.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Get the number of images
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// Calculate total size of all images
    pub fn total_size(&self) -> i64 {
        self.images.iter().map(|i| i.size).sum()
    }

    /// Format size in human readable format
    fn format_size(size: i64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        if size == 0 {
            return "0 B".to_string();
        }
        let size = size as f64;
        let exp = (size.ln() / 1024_f64.ln()).min(UNITS.len() as f64 - 1.0) as usize;
        let size = size / 1024_f64.powi(exp as i32);
        format!("{:.1} {}", size, UNITS[exp])
    }

    /// Build the table widget
    pub fn build_table(&self) -> Table<'_> {
        let header = Row::new(vec!["REPOSITORY", "TAG", "ID", "SIZE", "CREATED"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(0);

        let rows: Vec<Row> = self
            .images
            .iter()
            .map(|i| {
                // Get repository and tag from repo_tags
                let (repo, tag) = if i.dangling {
                    ("<none>".to_string(), "<none>".to_string())
                } else if let Some(repo_tag) = i.repo_tags.first() {
                    if let Some(pos) = repo_tag.rfind(':') {
                        let (r, t) = repo_tag.split_at(pos);
                        (r.to_string(), t[1..].to_string())
                    } else {
                        (repo_tag.clone(), "latest".to_string())
                    }
                } else {
                    ("<none>".to_string(), "<none>".to_string())
                };

                // Style for dangling images
                let style = if i.dangling {
                    theme::warn()
                } else {
                    Style::default()
                };

                let created = format_relative_time(i.created);

                Row::new(vec![
                    Line::from(Span::styled(repo, style)),
                    Line::from(Span::styled(tag, style)),
                    Line::from(i.short_id.clone()),
                    Line::from(Self::format_size(i.size)),
                    Line::from(created),
                ])
            })
            .collect();

        Table::new(
            rows,
            [
                Constraint::Min(20),    // Repository
                Constraint::Length(15), // Tag
                Constraint::Length(12), // ID
                Constraint::Length(10), // Size
                Constraint::Length(12), // Created
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(" Images ({}) ", self.images.len()))
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

/// Format a datetime as relative time (e.g., "2 hours ago")
fn format_relative_time(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(dt);

    if diff.num_days() > 365 {
        format!("{}y ago", diff.num_days() / 365)
    } else if diff.num_days() > 30 {
        format!("{}mo ago", diff.num_days() / 30)
    } else if diff.num_days() > 0 {
        format!("{}d ago", diff.num_days())
    } else if diff.num_hours() > 0 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_minutes() > 0 {
        format!("{}m ago", diff.num_minutes())
    } else {
        "just now".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_images() -> Vec<ImageSummary> {
        vec![
            ImageSummary {
                id: "abc123def456".to_string(),
                short_id: "abc123".to_string(),
                repo_tags: vec!["nginx:latest".to_string()],
                repo_digests: vec![],
                created: Utc::now(),
                size: 1024 * 1024 * 50, // 50 MB
                shared_size: 0,
                virtual_size: 1024 * 1024 * 50,
                labels: Default::default(),
                containers: 0,
                dangling: false,
                parent_id: "".to_string(),
            },
            ImageSummary {
                id: "def789abc012".to_string(),
                short_id: "def789".to_string(),
                repo_tags: vec![],
                repo_digests: vec![],
                created: Utc::now(),
                size: 1024 * 1024 * 100, // 100 MB
                shared_size: 0,
                virtual_size: 1024 * 1024 * 100,
                labels: Default::default(),
                containers: 0,
                dangling: true,
                parent_id: "".to_string(),
            },
        ]
    }

    #[test]
    fn test_image_list_creation() {
        let images = create_test_images();
        let widget = ImageListWidget::new(images);
        assert_eq!(widget.len(), 2);
        assert!(!widget.is_empty());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(ImageListWidget::format_size(0), "0 B");
        assert_eq!(ImageListWidget::format_size(1024), "1.0 KB");
        assert_eq!(ImageListWidget::format_size(1024 * 1024), "1.0 MB");
        assert_eq!(ImageListWidget::format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_navigation() {
        let images = create_test_images();
        let mut widget = ImageListWidget::new(images);

        assert_eq!(widget.state.selected(), Some(0));

        widget.next();
        assert_eq!(widget.state.selected(), Some(1));

        widget.next();
        assert_eq!(widget.state.selected(), Some(0)); // Wrap around

        widget.previous();
        assert_eq!(widget.state.selected(), Some(1)); // Wrap around
    }
}

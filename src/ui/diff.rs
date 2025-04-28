use ratatui::{
    prelude::*,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, Widget},
};
use similar::{ChangeTag, TextDiff};
use std::fmt;

/// Our own Change struct that wraps similar::Change functionality
#[derive(Debug, Clone)]
struct OurChange {
    pub tag: ChangeTag,
    pub value: String,
}

impl OurChange {
    pub fn tag(&self) -> ChangeTag {
        self.tag
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Represents different diff view modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiffViewMode {
    SideBySide,
    Inline,
    Unified,
}

/// Diff rendering configuration
#[derive(Debug, Clone)]
pub struct DiffConfig {
    pub view_mode: DiffViewMode,
    pub context_lines: usize,
    pub max_line_width: usize,
    pub syntax_highlight: bool,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            view_mode: DiffViewMode::Inline,
            context_lines: 3,
            max_line_width: 120,
            syntax_highlight: true,
        }
    }
}

/// Diff visualization state
#[derive(Debug)]
pub struct DiffVisualization {
    old_content: String,
    new_content: String,
    diff: Vec<OurChange>,
    config: DiffConfig,
    scroll_offset: usize,
}

impl DiffVisualization {
    /// Create a new diff visualization
    pub fn new(old_content: String, new_content: String) -> Self {
        // Create our own Change struct that wraps the similar crate's functionality
        // since the fields of similar::Change are private
        let diff = TextDiff::from_lines(&old_content, &new_content)
            .iter_all_changes()
            .map(|change| OurChange {
                tag: change.tag(),
                value: change.value().to_string(),
            })
            .collect();

        Self {
            old_content,
            new_content,
            diff,
            config: DiffConfig::default(),
            scroll_offset: 0,
        }
    }

    /// Set diff configuration
    pub fn with_config(mut self, config: DiffConfig) -> Self {
        self.config = config;
        self
    }

    /// Render diff based on current view mode
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        match self.config.view_mode {
            DiffViewMode::Inline => self.render_inline(area, buf),
            DiffViewMode::SideBySide => self.render_side_by_side(area, buf),
            DiffViewMode::Unified => self.render_unified(area, buf),
        }
    }

    /// Render inline diff view
    fn render_inline(&self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self
            .diff
            .iter()
            .filter_map(|change| match change.tag() {
                ChangeTag::Delete => Some(Line::from(vec![Span::styled(
                    format!("- {}", change.value()),
                    Style::default().fg(Color::Red),
                )])),
                ChangeTag::Insert => Some(Line::from(vec![Span::styled(
                    format!("+ {}", change.value()),
                    Style::default().fg(Color::Green),
                )])),
                ChangeTag::Equal => Some(Line::from(change.value())),
            })
            .collect();

        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Inline Diff"))
            .wrap(Wrap { trim: false });

        Widget::render(paragraph, area, buf);
    }

    /// Render side-by-side diff view
    fn render_side_by_side(&self, area: Rect, buf: &mut Buffer) {
        let half_width = area.width / 2;
        let left_area = Rect {
            x: area.x,
            y: area.y,
            width: half_width,
            height: area.height,
        };
        let right_area = Rect {
            x: area.x + half_width,
            y: area.y,
            width: half_width,
            height: area.height,
        };

        let old_lines: Vec<Line> = self
            .diff
            .iter()
            .filter_map(|change| match change.tag() {
                ChangeTag::Delete => Some(Line::from(vec![Span::styled(
                    format!("- {}", change.value()),
                    Style::default().fg(Color::Red),
                )])),
                ChangeTag::Equal => Some(Line::from(change.value())),
                _ => None,
            })
            .collect();

        let new_lines: Vec<Line> = self
            .diff
            .iter()
            .filter_map(|change| match change.tag() {
                ChangeTag::Insert => Some(Line::from(vec![Span::styled(
                    format!("+ {}", change.value()),
                    Style::default().fg(Color::Green),
                )])),
                ChangeTag::Equal => Some(Line::from(change.value())),
                _ => None,
            })
            .collect();

        let old_paragraph = Paragraph::new(old_lines)
            .block(Block::default().borders(Borders::ALL).title("Original"));
        let new_paragraph = Paragraph::new(new_lines)
            .block(Block::default().borders(Borders::ALL).title("Modified"));

        Widget::render(old_paragraph, left_area, buf);
        Widget::render(new_paragraph, right_area, buf);
    }

    /// Render unified diff view
    fn render_unified(&self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self
            .diff
            .iter()
            .enumerate()
            .filter_map(|(i, change)| match change.tag() {
                ChangeTag::Delete => Some(Line::from(vec![Span::styled(
                    format!("-{}: {}", i, change.value()),
                    Style::default().fg(Color::Red),
                )])),
                ChangeTag::Insert => Some(Line::from(vec![Span::styled(
                    format!("+{}: {}", i, change.value()),
                    Style::default().fg(Color::Green),
                )])),
                ChangeTag::Equal => Some(Line::from(format!(" {}: {}", i, change.value()))),
            })
            .collect();

        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Unified Diff"))
            .wrap(Wrap { trim: false });

        Widget::render(paragraph, area, buf);
    }

    /// Navigate diff view
    pub fn scroll(&mut self, delta: isize) {
        let new_offset = self.scroll_offset as isize + delta;
        self.scroll_offset = new_offset.max(0) as usize;
    }

    /// Get total number of changes
    pub fn total_changes(&self) -> usize {
        self.diff.len()
    }
}

/// Error handling for diff operations
#[derive(Debug)]
pub enum DiffError {
    ContentReadError,
    DiffGenerationError,
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffError::ContentReadError => write!(f, "Failed to read file contents"),
            DiffError::DiffGenerationError => write!(f, "Failed to generate diff"),
        }
    }
}

impl std::error::Error for DiffError {}

/// Public API for creating and managing diffs
pub fn create_diff(
    old_content: &str,
    new_content: &str,
    config: Option<DiffConfig>,
) -> Result<DiffVisualization, DiffError> {
    let diff = DiffVisualization::new(old_content.to_string(), new_content.to_string());

    Ok(match config {
        Some(cfg) => diff.with_config(cfg),
        None => diff,
    })
}

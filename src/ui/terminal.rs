use crossterm::{
    cursor,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Color as CrosstermColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::shell::terminal::{Terminal, TerminalInstance};

/// Represents a terminal view configuration
#[derive(Clone)]
pub struct TerminalView {
    pub instance: TerminalInstance,
    pub scroll_offset: usize,
    pub command_buffer: String,
    pub cursor_position: usize,
    pub history_index: Option<usize>,
    pub history: VecDeque<String>,
    pub suggestions: Vec<String>,
    pub selected_suggestion: Option<usize>,
}

/// Manages multiple terminal views and their layout
pub struct TerminalViewManager {
    views: Vec<TerminalView>,
    active_view: usize,
    layout: TerminalLayout,
    command_history: Arc<Mutex<HashMap<Uuid, VecDeque<String>>>>,
    max_history: usize,
    terminal: Arc<dyn Terminal>,
}

/// Defines how terminal views are arranged
#[derive(Clone, Copy)]
pub enum TerminalLayout {
    Single,
    HorizontalSplit,
    VerticalSplit,
    Grid,
}

impl TerminalView {
    pub fn new(instance: TerminalInstance) -> Self {
        Self {
            instance,
            scroll_offset: 0,
            command_buffer: String::new(),
            cursor_position: 0,
            history_index: None,
            history: VecDeque::with_capacity(1000),
            suggestions: Vec::new(),
            selected_suggestion: None,
        }
    }

    fn update_suggestions(&mut self) {
        if self.command_buffer.is_empty() {
            self.suggestions.clear();
            self.selected_suggestion = None;
            return;
        }

        // Generate suggestions based on command history and current input
        self.suggestions = self
            .history
            .iter()
            .filter(|cmd| cmd.starts_with(&self.command_buffer))
            .cloned()
            .collect();

        // Add common command suggestions
        let common_commands = vec!["cd", "ls", "git", "cargo", "vim", "cat", "grep", "find"];

        for cmd in common_commands {
            if cmd.starts_with(&self.command_buffer) && !self.suggestions.contains(&cmd.to_string())
            {
                self.suggestions.push(cmd.to_string());
            }
        }

        self.selected_suggestion = if self.suggestions.is_empty() {
            None
        } else {
            Some(0)
        };
    }
}

impl TerminalViewManager {
    pub fn new(terminal: Arc<dyn Terminal>) -> Self {
        Self {
            views: Vec::new(),
            active_view: 0,
            layout: TerminalLayout::Single,
            command_history: Arc::new(Mutex::new(HashMap::new())),
            max_history: 1000,
            terminal,
        }
    }

    pub fn add_view(&mut self, instance: TerminalInstance) {
        let view = TerminalView::new(instance.clone());
        self.views.push(view);

        // Create history entry for new instance
        if let Ok(mut history) = self.command_history.lock() {
            history.insert(instance.id(), VecDeque::with_capacity(self.max_history));
        }
    }

    pub fn cycle_layout(&mut self) {
        self.layout = match self.layout {
            TerminalLayout::Single => TerminalLayout::HorizontalSplit,
            TerminalLayout::HorizontalSplit => TerminalLayout::VerticalSplit,
            TerminalLayout::VerticalSplit => TerminalLayout::Grid,
            TerminalLayout::Grid => TerminalLayout::Single,
        };
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        if self.views.is_empty() {
            return false;
        }

        let view = &mut self.views[self.active_view];

        match (key.modifiers, key.code) {
            // Terminal switching
            (KeyModifiers::CONTROL, KeyCode::Tab) => {
                self.active_view = (self.active_view + 1) % self.views.len();
                true
            }

            // Command history navigation
            (KeyModifiers::NONE, KeyCode::Up) => {
                if let Some(idx) = view.history_index {
                    if idx + 1 < view.history.len() {
                        view.history_index = Some(idx + 1);
                        view.command_buffer = view.history[idx + 1].clone();
                        view.cursor_position = view.command_buffer.len();
                    }
                } else if !view.history.is_empty() {
                    view.history_index = Some(0);
                    view.command_buffer = view.history[0].clone();
                    view.cursor_position = view.command_buffer.len();
                }
                true
            }

            (KeyModifiers::NONE, KeyCode::Down) => {
                if let Some(idx) = view.history_index {
                    if idx > 0 {
                        view.history_index = Some(idx - 1);
                        view.command_buffer = view.history[idx - 1].clone();
                    } else {
                        view.history_index = None;
                        view.command_buffer.clear();
                    }
                    view.cursor_position = view.command_buffer.len();
                }
                true
            }

            // Suggestion navigation
            (KeyModifiers::CONTROL, KeyCode::Char('n')) => {
                if let Some(idx) = view.selected_suggestion {
                    if idx + 1 < view.suggestions.len() {
                        view.selected_suggestion = Some(idx + 1);
                    }
                }
                true
            }

            (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
                if let Some(idx) = view.selected_suggestion {
                    if idx > 0 {
                        view.selected_suggestion = Some(idx - 1);
                    }
                }
                true
            }

            // Accept suggestion
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if let Some(idx) = view.selected_suggestion {
                    view.command_buffer = view.suggestions[idx].clone();
                    view.cursor_position = view.command_buffer.len();
                    view.update_suggestions();
                }
                true
            }

            // Command editing
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                view.command_buffer.insert(view.cursor_position, c);
                view.cursor_position += 1;
                view.history_index = None;
                view.update_suggestions();
                true
            }

            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if view.cursor_position > 0 {
                    view.command_buffer.remove(view.cursor_position - 1);
                    view.cursor_position -= 1;
                    view.history_index = None;
                    view.update_suggestions();
                }
                true
            }

            _ => false,
        }
    }

    pub fn draw(&self, f: &mut Frame) {
        let chunks = self.get_layout_chunks(f.area());

        for (i, view) in self.views.iter().enumerate() {
            if i >= chunks.len() {
                break;
            }

            let is_active = i == self.active_view;
            self.draw_view(f, view, chunks[i], is_active);
        }
    }

    fn get_layout_chunks(&self, area: Rect) -> Vec<Rect> {
        match self.layout {
            TerminalLayout::Single => Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(area)
                .to_vec(),

            TerminalLayout::HorizontalSplit => Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(area)
                .to_vec(),

            TerminalLayout::VerticalSplit => Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(area)
                .to_vec(),

            TerminalLayout::Grid => {
                let horizontal = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                    .split(area)
                    .to_vec();

                let mut chunks = Vec::new();
                for chunk in horizontal {
                    chunks.extend(
                        Layout::default()
                            .direction(Direction::Vertical)
                            .constraints(
                                [Constraint::Percentage(50), Constraint::Percentage(50)].as_ref(),
                            )
                            .split(chunk)
                            .to_vec(),
                    );
                }
                chunks
            }
        }
    }

    fn draw_view(&self, f: &mut Frame, view: &TerminalView, area: Rect, is_active: bool) {
        let border_style = if is_active {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };

        // Create border
        let block = Block::default()
            .title(view.instance.title.as_str())
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        f.render_widget(block, area);

        // Split inner area into output and input areas
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
            .split(inner_area);

        // Draw suggestions if any
        if !view.suggestions.is_empty() && is_active {
            let suggestion_items: Vec<ListItem> = view
                .suggestions
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let style = if Some(i) == view.selected_suggestion {
                        Style::default().fg(Color::Black).bg(Color::White)
                    } else {
                        Style::default()
                    };
                    ListItem::new(s.as_str()).style(style)
                })
                .collect();

            let suggestions_list =
                List::new(suggestion_items).block(Block::default().borders(Borders::ALL));

            let suggestion_area = Rect {
                height: (view.suggestions.len() as u16).min(5),
                y: chunks[1].y - (view.suggestions.len() as u16).min(5),
                ..chunks[1]
            };

            f.render_widget(suggestions_list, suggestion_area);
        }

        // Draw command input
        let input =
            Paragraph::new(view.command_buffer.as_str()).style(Style::default().fg(Color::White));
        f.render_widget(input, chunks[1]);

        // Draw cursor if active
        if is_active {
            f.set_cursor_position((chunks[1].x + view.cursor_position as u16, chunks[1].y));
        }
    }
}

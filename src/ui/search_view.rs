use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget, Wrap},
    Frame,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::search::{SearchManager, SearchMatch, SearchOptions};

#[derive(Default, Debug)]
pub struct SearchState {
    query: String,
    results: Vec<SearchMatch>,
    selected_result: Option<usize>,
    options: SearchOptions,
    history: Vec<String>,
    history_index: Option<usize>,
}

impl std::fmt::Debug for SearchView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchView")
            .field("state", &"Arc<Mutex<SearchState>>")
            .field("manager", &"Arc<SearchManager>")
            .finish()
    }
}

pub struct SearchView {
    state: Arc<Mutex<SearchState>>,
    manager: Arc<SearchManager>,
}

impl SearchView {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(SearchState::default())),
            manager: Arc::new(SearchManager::new()),
        }
    }

    pub async fn set_query(&self, query: String) {
        let mut state = self.state.lock().await;
        if !query.is_empty() && (state.history.is_empty() || state.history[0] != query) {
            state.history.insert(0, query.clone());
            if state.history.len() > 50 {
                state.history.pop();
            }
        }
        state.query = query;
        state.selected_result = None;
    }

    pub async fn toggle_case_sensitive(&self) {
        let mut state = self.state.lock().await;
        state.options.case_sensitive = !state.options.case_sensitive;
        self.manager.set_options(state.options.clone()).await;
    }

    pub async fn toggle_regex_mode(&self) {
        let mut state = self.state.lock().await;
        state.options.regex_mode = !state.options.regex_mode;
        self.manager.set_options(state.options.clone()).await;
    }

    pub async fn navigate_history(&self, direction: isize) {
        let mut state = self.state.lock().await;
        if state.history.is_empty() {
            return;
        }

        let new_index = match (state.history_index, direction) {
            (None, 1) => Some(0),
            (Some(i), 1) if i + 1 < state.history.len() => Some(i + 1),
            (Some(i), -1) if i > 0 => Some(i - 1),
            (Some(_), -1) => None,
            _ => state.history_index,
        };

        state.history_index = new_index;
        if let Some(i) = new_index {
            state.query = state.history[i].clone();
        }
    }

    pub async fn select_next_result(&self) {
        let mut state = self.state.lock().await;
        if state.results.is_empty() {
            return;
        }

        state.selected_result = Some(match state.selected_result {
            None => 0,
            Some(i) if i + 1 < state.results.len() => i + 1,
            Some(i) => i,
        });
    }

    pub async fn select_previous_result(&self) {
        let mut state = self.state.lock().await;
        if let Some(i) = state.selected_result {
            if i > 0 {
                state.selected_result = Some(i - 1);
            }
        }
    }

    pub async fn search(&self, text: &str) {
        let mut state = self.state.lock().await;
        if state.query.is_empty() {
            state.results.clear();
            return;
        }

        let search_result = self.manager.search(text, &state.query).await;
        state.results = search_result.matches;
        state.selected_result = if state.results.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search input
                Constraint::Length(3), // Search options
                Constraint::Min(0),    // Results
            ])
            .split(area);

        // We need to clone state for rendering since we can't hold the lock
        // across the entire render operation
        let state = self.state.blocking_lock();

        // Render search input
        let input = Paragraph::new(state.query.as_str())
            .style(Style::default())
            .block(Block::default().borders(Borders::ALL).title("Search"));
        frame.render_widget(input, chunks[0]);

        // Render search options
        let options = vec![
            format!("[C]ase-sensitive: {}", if state.options.case_sensitive { "On" } else { "Off" }),
            format!("[R]egex: {}", if state.options.regex_mode { "On" } else { "Off" }),
            format!("Results: {}", state.results.len()),
        ];
        let options = Paragraph::new(Text::from(options.join(" | ")))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(options, chunks[1]);

        // Render search results
        let mut results = Vec::new();
        for (idx, result) in state.results.iter().enumerate() {
            let is_selected = state.selected_result == Some(idx);
            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default()
            };

            // Add context before
            for context in &result.context_before {
                results.push(ListItem::new(Line::from(vec![
                    Span::styled("  ", style),
                    Span::styled(context, Style::default().fg(Color::DarkGray)),
                ])));
            }

            // Add matched line with highlighting
            let mut line = Vec::new();
            line.push(Span::styled(
                format!("{:>3} ", result.line_number),
                Style::default().fg(Color::Yellow),
            ));

            let content = &result.line_content;
            if result.length > 0 {
                // Add content before match
                if result.start_pos > 0 {
                    line.push(Span::raw(&content[..result.start_pos]));
                }
                // Add highlighted match
                line.push(Span::styled(
                    &content[result.start_pos..result.start_pos + result.length],
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                // Add content after match
                if result.start_pos + result.length < content.len() {
                    line.push(Span::raw(&content[result.start_pos + result.length..]));
                }
            } else {
                line.push(Span::raw(content));
            }

            results.push(ListItem::new(Line::from(line)).style(style));

            // Add context after
            for context in &result.context_after {
                results.push(ListItem::new(Line::from(vec![
                    Span::styled("  ", style),
                    Span::styled(context, Style::default().fg(Color::DarkGray)),
                ])));
            }
        }

        let results = List::new(results)
            .block(Block::default().borders(Borders::ALL).title("Results"))
            .highlight_style(Style::default().bg(Color::White).fg(Color::Black));

        frame.render_widget(results, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_view_creation() {
        let view = SearchView::new();
        let state = view.state.lock().await;
        assert!(state.query.is_empty());
        assert!(state.results.is_empty());
        assert!(state.selected_result.is_none());
    }

    #[tokio::test]
    async fn test_search_history() {
        let view = SearchView::new();
        
        // Add some searches
        view.set_query("first".to_string()).await;
        view.set_query("second".to_string()).await;
        view.set_query("third".to_string()).await;

        let state = view.state.lock().await;
        assert_eq!(state.history.len(), 3);
        assert_eq!(state.history[0], "third");
        assert_eq!(state.history[1], "second");
        assert_eq!(state.history[2], "first");
    }

    #[tokio::test]
    async fn test_result_navigation() {
        let view = SearchView::new();
        let mut state = view.state.lock().await;
        
        // Add some mock results
        state.results = vec![
            SearchMatch {
                line_number: 1,
                line_content: "first match".to_string(),
                start_pos: 0,
                length: 5,
                context_before: vec![],
                context_after: vec![],
            },
            SearchMatch {
                line_number: 2,
                line_content: "second match".to_string(),
                start_pos: 0,
                length: 6,
                context_before: vec![],
                context_after: vec![],
            },
        ];
        drop(state);

        // Test navigation
        view.select_next_result().await;
        assert_eq!(view.state.lock().await.selected_result, Some(0));
        
        view.select_next_result().await;
        assert_eq!(view.state.lock().await.selected_result, Some(1));
        
        view.select_previous_result().await;
        assert_eq!(view.state.lock().await.selected_result, Some(0));
    }
}
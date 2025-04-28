use ratatui::{
    Frame,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::ui::app::{App, MainViewType};
use crate::ui::input::InputMode;

/// Renders the main user interface
pub fn render_ui(f: &mut Frame, app: &App) {
    // Calculate the height needed for the input area based on content
    let input_height = calculate_input_height(&app.input_text, f.area().width);

    // Create a layout with 3 vertical sections
    // Main area + Chat view (horizontally split at the top)
    // Command input area (at the bottom, can be multiline)
    // Keyboard shortcut area (at the very bottom)
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),               // Main + Chat area
            Constraint::Length(input_height), // Command input (resizes based on content)
            Constraint::Length(1),            // Keyboard shortcuts
        ])
        .split(f.area());

    // Split the top area horizontally for Main View and Chat View
    let top_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Main view
            Constraint::Percentage(40), // Chat view
        ])
        .split(main_layout[0]);

    // Render different components
    render_main_view(f, app, top_areas[0]);
    render_chat_view(f, app, top_areas[1]);
    render_input_area(f, app, main_layout[1]);
    render_shortcut_area(f, app, main_layout[2]);
}

/// Renders the main view area based on current view type
fn render_main_view(f: &mut Frame, app: &App, area: Rect) {
    let title = match app.current_main_view {
        MainViewType::FileTree => "File Tree",
        MainViewType::GitDiff => "Git Diff",
        MainViewType::ShellOutput => "Shell Output",
        MainViewType::LlmResponse => "LLM Response",
        MainViewType::Search => "Search Results",
        MainViewType::CodeOutline => "Code Outline",
    };

    // Create a styled block for the main view
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    f.render_widget(block, area);

    // Render content based on the current view type
    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    match app.current_main_view {
        MainViewType::FileTree => {
            // Placeholder for file tree rendering
            let text = vec![
                Line::from("📁 src/"),
                Line::from("  📁 ui/"),
                Line::from("    📄 app.rs"),
                Line::from("    📄 input.rs"),
                Line::from("    📄 mod.rs"),
                Line::from("  📄 main.rs"),
            ];
            let paragraph = Paragraph::new(text).wrap(Wrap { trim: true });
            f.render_widget(paragraph, inner_area);
        }
        MainViewType::GitDiff => {
            // Placeholder for git diff rendering
            let text = vec![
                Line::from(vec![Span::styled(
                    "diff --git a/src/main.rs b/src/main.rs",
                    Style::default().fg(Color::White),
                )]),
                Line::from(vec![Span::styled(
                    "--- a/src/main.rs",
                    Style::default().fg(Color::White),
                )]),
                Line::from(vec![Span::styled(
                    "+++ b/src/main.rs",
                    Style::default().fg(Color::White),
                )]),
                Line::from(vec![Span::styled(
                    "@@ -1,5 +1,7 @@",
                    Style::default().fg(Color::Cyan),
                )]),
                Line::from(vec![Span::styled(
                    "-fn main() {",
                    Style::default().fg(Color::Red),
                )]),
                Line::from(vec![Span::styled(
                    "+use std::io;",
                    Style::default().fg(Color::Green),
                )]),
                Line::from(vec![Span::styled("+", Style::default().fg(Color::Green))]),
                Line::from(vec![Span::styled(
                    "+fn main() -> Result<(), io::Error> {",
                    Style::default().fg(Color::Green),
                )]),
            ];
            let paragraph = Paragraph::new(text);
            f.render_widget(paragraph, inner_area);
        }
        MainViewType::ShellOutput => {
            // Get the most recent shell output from chat history
            let empty_string = String::new();
            let shell_output = app.chat_messages.iter()
                .rev()
                .find(|msg| !msg.is_user)
                .map(|msg| &msg.content)
                .unwrap_or(&empty_string);
                
            // Convert shell output to lines
            let text: Vec<Line> = shell_output
                .lines()
                .map(|line| {
                    // Special handling for directory trees
                    if line.contains("├") || line.contains("└") || line.contains("│") {
                        Line::from(Span::styled(line, Style::default().fg(Color::Cyan)))
                    } else if line.starts_with("$") || line.starts_with("#") {
                        Line::from(Span::styled(line, Style::default().fg(Color::Yellow)))
                    } else {
                        Line::from(line)
                    }
                })
                .collect();
                
            let paragraph = Paragraph::new(text)
                .style(Style::default().fg(Color::Gray))
                .wrap(Wrap { trim: false }); // Don't trim to preserve tree structure
                
            f.render_widget(paragraph, inner_area);
        }
        MainViewType::LlmResponse => {
            // Placeholder for LLM response rendering
            let text = vec![
                Line::from(vec![Span::styled(
                    "# Example Markdown Response",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from("This is an example of an LLM response with markdown formatting."),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "## Code Example",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from("```rust"),
                Line::from("fn main() {"),
                Line::from("    println!(\"Hello, world!\");"),
                Line::from("}"),
                Line::from("```"),
            ];
            let paragraph = Paragraph::new(text).wrap(Wrap { trim: true });
            f.render_widget(paragraph, inner_area);
        }
        MainViewType::Search => {
            // Placeholder for search rendering
            let text = vec![
                Line::from(vec![
                    Span::styled("Search Results for: ", Style::default().fg(Color::White)),
                    Span::styled("\"println\"", Style::default().fg(Color::Yellow)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("src/main.rs:5: ", Style::default().fg(Color::Blue)),
                    Span::styled("    ", Style::default()),
                    Span::styled(
                        "println",
                        Style::default().bg(Color::Yellow).fg(Color::Black),
                    ),
                    Span::styled("!(\"Hello, world!\");", Style::default()),
                ]),
                Line::from(vec![
                    Span::styled("src/ui/app.rs:42: ", Style::default().fg(Color::Blue)),
                    Span::styled("    ", Style::default()),
                    Span::styled(
                        "println",
                        Style::default().bg(Color::Yellow).fg(Color::Black),
                    ),
                    Span::styled("!(\"UI initialized\");", Style::default()),
                ]),
            ];
            let paragraph = Paragraph::new(text);
            f.render_widget(paragraph, inner_area);
        }
        MainViewType::CodeOutline => {
            // Placeholder for code outline rendering
            let text = vec![
                Line::from(vec![
                    Span::styled("Code Outline for: ", Style::default().fg(Color::White)),
                    Span::styled("src/main.rs", Style::default().fg(Color::Yellow)),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("fn main() [1-10]", Style::default().fg(Color::Cyan))]),
                Line::from(vec![Span::styled(
                    "  fn setup_app() [3-5]",
                    Style::default().fg(Color::Blue),
                )]),
                Line::from(vec![Span::styled(
                    "  fn run_app() [7-9]",
                    Style::default().fg(Color::Blue),
                )]),
                Line::from(vec![Span::styled(
                    "struct AppConfig [12-20]",
                    Style::default().fg(Color::Green),
                )]),
                Line::from(vec![Span::styled(
                    "  fn new() [14-16]",
                    Style::default().fg(Color::Blue),
                )]),
                Line::from(vec![Span::styled(
                    "  fn load() [18-20]",
                    Style::default().fg(Color::Blue),
                )]),
            ];
            let paragraph = Paragraph::new(text);
            f.render_widget(paragraph, inner_area);
        }
    }
}

/// Renders the chat view area with message history
fn render_chat_view(f: &mut Frame, app: &App, area: Rect) {
    // Create a styled block for the chat view
    let block = Block::default()
        .title("Chat History")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    f.render_widget(block, area);

    // Calculate inner area for the chat content
    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Prepare chat messages for display
    let mut all_lines: Vec<Line> = Vec::new();
    
    for msg in app.chat_messages.iter() {
        // Add the role label first
        if msg.is_user {
            all_lines.push(Line::from(vec![
                Span::styled(
                    "You: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else {
            all_lines.push(Line::from(vec![
                Span::styled(
                    "Assistant: ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        
        // Split the content into lines and add each one
        for content_line in msg.content.lines() {
            all_lines.push(Line::from(Span::raw(content_line)));
        }
        
        // Add a blank line after each message for spacing
        all_lines.push(Line::from(""));
    }
    
    // Calculate scroll position to show the most recent messages if they don't all fit
    let scroll_offset = if all_lines.len() as u16 > inner_area.height {
        (all_lines.len() as u16).saturating_sub(inner_area.height)
    } else {
        0
    };
    
    let chat_content = Paragraph::new(all_lines)
        .wrap(Wrap { trim: true })
        .scroll((scroll_offset, 0));

    f.render_widget(chat_content, inner_area);
}

/// Renders the command input area
fn render_input_area(f: &mut Frame, app: &App, area: Rect) {
    // Create a styled block for the input area
    let input_title = match app.input_mode {
        InputMode::Normal => "Input",
        InputMode::Command => "Command Mode",
        InputMode::Search => "Search Mode",
        InputMode::Diff => "Diff Mode",
        InputMode::Help => "Help Mode",
    };

    let block = Block::default()
        .title(input_title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    f.render_widget(block, area);

    // Render the input text
    let input_text =
        Paragraph::new(app.input_text.clone()).style(Style::default().fg(Color::White));

    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    f.render_widget(input_text, inner_area);

    // Set cursor position, ensuring it's valid
    // We need to add 1 to account for the block border
    // Make sure cursor_position is within valid text boundaries and convert to screen position
    let cursor_screen_pos = if app.cursor_position <= app.input_text.len() {
        // Calculate column by counting displayed characters up to the cursor position
        // For simplicity, we're assuming each character takes 1 column
        // For a more accurate implementation, you'd need to consider grapheme clusters
        let cursor_text = app.input_text.chars().take(app.cursor_position).collect::<String>();
        inner_area.x + cursor_text.chars().count() as u16
    } else {
        // Fallback if cursor is somehow out of bounds
        inner_area.x
    };
    
    f.set_cursor_position((cursor_screen_pos, inner_area.y));
}

/// Renders the keyboard shortcut area
fn render_shortcut_area(f: &mut Frame, app: &App, area: Rect) {
    // Create shortcut text based on current mode
    let shortcuts = match app.input_mode {
        InputMode::Normal => "! bash  / command  @ file  ? help  Ctrl+Q quit",
        InputMode::Command => "Esc back  Tab complete  Enter submit",
        InputMode::Search => "Esc back  ↑↓ navigate  Enter select",
        InputMode::Diff => "Esc back  j/k scroll  f toggle fold",
        InputMode::Help => "Esc back  ↑↓ navigate  q close",
    };

    let shortcut_text = Paragraph::new(shortcuts).style(Style::default().fg(Color::DarkGray));

    f.render_widget(shortcut_text, area);
}

/// Calculate the needed height for multiline input
fn calculate_input_height(input: &str, width: u16) -> u16 {
    let line_count = if input.is_empty() {
        1
    } else {
        input.lines().count() as u16 + 
        // Add extra lines for wrapped content
        input.lines()
            .map(|line| (line.len() as u16).saturating_sub(1) / (width.saturating_sub(2)) + 1)
            .sum::<u16>()
            .saturating_sub(input.lines().count() as u16)
    };

    // Height is min 1, max 10, plus 2 for borders
    2 + line_count.clamp(1, 10)
}

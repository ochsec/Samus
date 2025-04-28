use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, Widget},
};

use crate::ui::app::{App, MainViewType};
use crate::ui::input::InputMode;

/// Renders the main user interface
pub fn render_ui(f: &mut Frame, app: &mut App) {
    // Determine the available area
    let area = f.size();
    
    // If we're displaying a completion, use the full screen for the main view
    if app.displaying_completion {
        // Use the entire terminal area for the main view
        render_main_view(f, app, area);
        return;
    }
    
    // Otherwise, show the input area and shortcuts
    // Calculate the height needed for the input area based on content
    let input_height = calculate_input_height(&app.input_text, area.width);

    // Create a layout with 3 vertical sections
    // Main view area (taking most of the screen)
    // Command input area (at the bottom, can be multiline)
    // Keyboard shortcut area (at the very bottom)
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),               // Main view area
            Constraint::Length(input_height), // Command input (resizes based on content)
            Constraint::Length(1),            // Keyboard shortcuts
        ])
        .split(area);

    // Render different components
    render_main_view(f, app, main_layout[0]);
    render_input_area(f, app, main_layout[1]);
    render_shortcut_area(f, app, main_layout[2]);
}

/// Renders the main view area based on current view type
fn render_main_view(f: &mut Frame, app: &mut App, area: Rect) {
    // No titles or borders for the main view to maximize content space
    let inner_area = area;

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
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: true });
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
            // Create a combined view of user inputs and responses
            let mut text: Vec<Line> = Vec::new();
            
            // Process all chat messages in order
            for msg in app.chat_messages.iter() {
                if msg.is_user {
                    // User message
                    text.push(Line::from(vec![
                        Span::styled(
                            "You: ",
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(&msg.content),
                    ]));
                    text.push(Line::from(""));
                } else {
                    // Assistant message
                    if msg.content == "Thinking..." {
                        // Skip "Thinking..." messages
                        continue;
                    }
                    
                    text.push(Line::from(vec![
                        Span::styled(
                            "Samus: ",
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    
                    // Process assistant response, with special handling for different content types
                    for line in msg.content.lines() {
                        // Special handling for directory trees
                        if line.contains("├") || line.contains("└") || line.contains("│") {
                            text.push(Line::from(Span::styled(line, Style::default().fg(Color::Cyan))));
                        } else if line.starts_with("$") || line.starts_with("#") {
                            text.push(Line::from(Span::styled(line, Style::default().fg(Color::Yellow))));
                        } else if line.starts_with("```") {
                            // Code block markers
                            text.push(Line::from(Span::styled(line, Style::default().fg(Color::Cyan))));
                        } else if line.starts_with("# ") || line.starts_with("## ") {
                            // Markdown headers
                            text.push(Line::from(Span::styled(
                                line,
                                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                            )));
                        } else {
                            text.push(Line::from(line));
                        }
                    }
                    
                    text.push(Line::from("")); // Add a blank line after each message
                }
            }
            
            // Show a scroll indicator at the bottom when there's content to scroll
            if text.len() as u16 > inner_area.height {
                // Add a note at the bottom of the visible content
                let scroll_info_line = Line::from(vec![
                    Span::styled(
                        "-- Scroll with terminal's scrollback (PgUp/PgDown or mouse wheel) --",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                
                // Add the scroll indicator to the list
                text.push(scroll_info_line);
            }
            
            // Create the main content paragraph without scrolling
            let paragraph = Paragraph::new(text)
                .style(Style::default().fg(Color::Gray))
                .wrap(Wrap { trim: false }); // Don't trim to preserve formatting
                
            f.render_widget(paragraph, inner_area);
        }
        MainViewType::LlmResponse => {
            // Get the most recent LLM response from chat history
            let empty_string = String::new();
            let llm_response = app.chat_messages.iter()
                .rev()
                .find(|msg| !msg.is_user && msg.content != "Thinking...")
                .map(|msg| &msg.content)
                .unwrap_or(&empty_string);
                
            // Convert LLM response to lines
            let text: Vec<Line> = llm_response
                .lines()
                .map(|line| {
                    // Basic formatting for markdown headers
                    if line.starts_with("# ") {
                        Line::from(vec![Span::styled(
                            line,
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        )])
                    } else if line.starts_with("## ") {
                        Line::from(vec![Span::styled(
                            line,
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        )])
                    } else if line.starts_with("```") {
                        Line::from(vec![Span::styled(
                            line,
                            Style::default().fg(Color::Cyan),
                        )])
                    } else {
                        Line::from(line)
                    }
                })
                .collect();
                
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: true });
                
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
            // Render actual symbols if available, otherwise placeholder
            let text = if !app.current_file_symbols.is_empty() {
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Code Outline for: ", Style::default().fg(Color::White)),
                        Span::styled(
                            app.current_file_path.as_deref().unwrap_or("Unknown"), 
                            Style::default().fg(Color::Yellow)
                        ),
                    ]),
                    Line::from(""),
                ];
                
                // Add each symbol
                for symbol in &app.current_file_symbols {
                    let color = match symbol.kind.as_str() {
                        "Function" | "Method" => Color::Cyan,
                        "Class" | "Struct" | "Interface" => Color::Green,
                        "Variable" | "Property" => Color::Blue,
                        _ => Color::White,
                    };
                    
                    lines.push(Line::from(vec![Span::styled(
                        format!("{} {} [line {}]", symbol.kind, symbol.name, symbol.line),
                        Style::default().fg(color),
                    )]));
                }
                
                lines
            } else {
                // Placeholder data
                vec![
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
                ]
            };
            
            let paragraph = Paragraph::new(text);
            f.render_widget(paragraph, inner_area);
        }
    }
}

/// This function is no longer used, but kept as a stub for compatibility
fn render_chat_view(_f: &mut Frame, _app: &mut App, _area: Rect) {
    // No longer used as we've merged the chat view into the main view
}

/// Renders the command input area
fn render_input_area(f: &mut Frame, app: &mut App, area: Rect) {
    // Create a styled block for the input area with rounded corners
    let input_title = match app.input_mode {
        InputMode::Normal => "",         // No title for normal mode
        InputMode::Command => "Command", // Simplified titles for other modes
        InputMode::Search => "Search",
        InputMode::Diff => "Diff",
        InputMode::Help => "Help",
    };

    let block = Block::default()
        .title(input_title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .border_type(ratatui::widgets::BorderType::Rounded); // Rounded corners

    f.render_widget(block, area);

    // Create the prompt and input text
    let prompt = "> ";
    let display_text = format!("{}{}", prompt, app.input_text);
    
    let input_text = Paragraph::new(display_text)
        .style(Style::default().fg(Color::White));

    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    f.render_widget(input_text, inner_area);

    // Set cursor position, accounting for the prompt
    let cursor_screen_pos = if app.cursor_position <= app.input_text.len() {
        // Calculate column by counting displayed characters up to the cursor position
        // Add the prompt length to the cursor position
        let cursor_text = app.input_text.chars().take(app.cursor_position).collect::<String>();
        inner_area.x + prompt.len() as u16 + cursor_text.chars().count() as u16
    } else {
        // Fallback if cursor is somehow out of bounds
        inner_area.x + prompt.len() as u16
    };
    
    f.set_cursor(cursor_screen_pos, inner_area.y);
}

/// Renders the keyboard shortcut area
fn render_shortcut_area(f: &mut Frame, app: &mut App, area: Rect) {
    // Create shortcut text based on current mode with a cleaner look
    let shortcuts = match app.input_mode {
        InputMode::Normal => if app.displaying_completion {
            "Esc show input  Ctrl+Q quit"  // When in full-screen mode
        } else {
            "! bash  / command  @ file  Esc fullscreen  Ctrl+Q quit"  // When input is visible
        },
        InputMode::Command => "Esc back  Tab complete  Enter submit",
        InputMode::Search => "Esc back  ↑↓ navigate  Enter select",
        InputMode::Diff => "Esc back  j/k scroll  f toggle fold",
        InputMode::Help => "Esc back  ↑↓ navigate  q close",
    };

    let shortcut_text = Paragraph::new(shortcuts)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(ratatui::layout::Alignment::Center); // Center align for a cleaner look

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
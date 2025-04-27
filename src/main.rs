mod task;
mod resource;
mod context;
mod error;
mod config;
mod mcp;
mod fs;
mod shell;
mod ui;
mod perf;
mod simple_client;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{io, time::Duration, time::Instant};
use std::error::Error;
use tokio::runtime::Runtime;
use dotenv::dotenv;

use crate::config::McpServerConfig;
use crate::ui::app::App;

/// Application entry point
fn main() -> Result<(), Box<dyn Error>> {
    // Load .env file
    dotenv().ok();
    
    // Create a tokio runtime
    let runtime = Runtime::new()?;
    
    // Uncomment to run the simple client instead of the TUI
    // println!("Running simple OpenRouter client instead of TUI...");
    // runtime.block_on(simple_client::run_simple_client())?;
    
    // Run the TUI code
    
    println!("Starting Samus TUI...");
    
    // Setup terminal
    match enable_raw_mode() {
        Ok(_) => {},
        Err(e) => {
            println!("Failed to enable raw mode: {}", e);
            return Err(Box::new(e));
        }
    }
    
    let mut stdout = io::stdout();
    if let Err(e) = stdout.execute(EnterAlternateScreen) {
        disable_raw_mode()?;
        println!("Failed to enter alternate screen: {}", e);
        return Err(Box::new(e));
    }
    
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(term) => term,
        Err(e) => {
            disable_raw_mode()?;
            println!("Failed to create terminal: {}", e);
            return Err(Box::new(e));
        }
    };
    
    // Create app state
    let mut app = App::new();
    
    // Add welcome message
    app.add_chat_message(
        "Welcome to Samus! Type a message or use a slash command like /help to get started.".to_string(), 
        false
    );
    
    // Configure OpenRouter
    if let Ok(api_key) = std::env::var("OPEN_ROUTER_API_KEY") {
        // Create config
        let config = McpServerConfig {
            id: "openrouter".to_string(),
            name: "OpenRouter".to_string(),
            url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
            api_key: Some(api_key),
            enabled: true,
        };
        
        // Initialize client
        match app.init_llm_client(config) {
            Ok(_) => app.add_chat_message("OpenRouter client configured successfully with Claude 3.5 Haiku.".to_string(), false),
            Err(e) => app.add_chat_message(format!("Error configuring OpenRouter client: {}", e), false),
        }
    } else {
        app.add_chat_message("OpenRouter API key not found. Set OPEN_ROUTER_API_KEY in .env or use /config <api_key>.".to_string(), false);
    }
    
    // Run the app
    let res = run_tui(&mut terminal, &mut app);
    
    // Restore terminal
    if let Err(e) = disable_raw_mode() {
        println!("Failed to disable raw mode: {}", e);
    }
    
    if let Err(e) = terminal.backend_mut().execute(LeaveAlternateScreen) {
        println!("Failed to leave alternate screen: {}", e);
    }
    
    if let Err(e) = terminal.show_cursor() {
        println!("Failed to show cursor: {}", e);
    }
    
    if let Err(err) = res {
        println!("App error: {}", err);
    }
    
    Ok(())
}

/// Run the TUI interface
fn run_tui<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
    
    // Show logo on startup
    terminal.draw(|f| {
        let logo_area = ratatui::layout::Rect::new(
            0, 
            f.area().height / 4, 
            f.area().width, 
            10
        );
        ui::render_logo(f, logo_area);
    })?;
    
    // Pause to show the logo
    std::thread::sleep(Duration::from_millis(1500));
    
    loop {
        terminal.draw(|f| {
            let area = f.area();
            
            // Draw header
            let header = ratatui::widgets::Paragraph::new("Samus TUI - Claude 3.5 Haiku")
                .style(ratatui::style::Style::default().fg(ratatui::style::Color::Cyan))
                .alignment(ratatui::layout::Alignment::Center);
            let header_chunk = ratatui::layout::Rect::new(0, 0, area.width, 1);
            f.render_widget(header, header_chunk);
            
            // Create chat area with border
            let chat_area = ratatui::widgets::Block::default()
                .title("Chat")
                .borders(ratatui::widgets::Borders::ALL);
            let chat_chunk = ratatui::layout::Rect::new(0, 1, area.width, area.height - 3);
            f.render_widget(chat_area.clone(), chat_chunk);
            
            // Render chat messages
            let chat_inner = chat_area.inner(chat_chunk);
            let mut y = chat_inner.y;
            for msg in &app.chat_messages {
                let prefix = if msg.is_user { "You: " } else { "AI: " };
                let style = if msg.is_user {
                    ratatui::style::Style::default().fg(ratatui::style::Color::Green)
                } else {
                    ratatui::style::Style::default().fg(ratatui::style::Color::Blue)
                };
                
                let text = format!("{}{}", prefix, msg.content);
                
                // Calculate height based on text wrapping
                let height = (text.len() as u16 / chat_inner.width).max(1) + 1;
                
                let para = ratatui::widgets::Paragraph::new(text)
                    .style(style)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                
                // Create a chunk for this message
                let msg_chunk = ratatui::layout::Rect::new(
                    chat_inner.x, 
                    y, 
                    chat_inner.width, 
                    height.min(chat_inner.height)
                );
                
                // Render if it fits
                if y + height <= chat_inner.y + chat_inner.height {
                    f.render_widget(para, msg_chunk);
                }
                
                y += height;
            }
            
            // Create input area with border
            let input_area = ratatui::widgets::Block::default()
                .title("Input")
                .borders(ratatui::widgets::Borders::ALL);
            let input_chunk = ratatui::layout::Rect::new(0, area.height - 3, area.width, 3);
            f.render_widget(input_area.clone(), input_chunk);
            
            // Render input text
            let input_text = if app.is_processing {
                "Processing...".to_string()
            } else {
                app.input_text.clone()
            };
            
            let input = ratatui::widgets::Paragraph::new(input_text)
                .style(ratatui::style::Style::default());
            let input_inner = input_area.inner(input_chunk);
            f.render_widget(input, input_inner);
            
            // Show cursor at input position
            if !app.is_processing {
                f.set_cursor_position(
                    ratatui::layout::Position {
                        x: input_inner.x + app.cursor_position as u16,
                        y: input_inner.y
                    }
                );
            }
        })?;
        
        // Check for events
        if crossterm::event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Check for quit command
                if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
                
                // Handle other key events
                if !app.is_processing {
                    app.handle_key_event(key);
                }
            }
        }
        
        // Handle ticks
        let now = Instant::now();
        if now.duration_since(last_tick) >= tick_rate {
            app.on_tick();
            last_tick = now;
        }
        
        // Check if we should exit
        if app.should_quit {
            break;
        }
    }
    
    Ok(())
}


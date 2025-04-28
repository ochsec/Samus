mod config;
mod context;
mod error;
mod fs;
mod integrations;
mod mcp;
mod perf;
mod resource;
mod services;
mod shell;
mod simple_client;
mod task;
mod tools;
mod ui;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dotenv::dotenv;
use std::{error::Error, io};

use crate::config::McpServerConfig;
use crate::services::tree_sitter::initialize_service;
use crate::task::{TaskRegistry, TaskManager};
use crate::task::tree_sitter_task::TreeSitterTaskHandler;
use crate::task::shell_task::ShellTaskHandler;
use crate::ui::app::App;
use crate::ui::tui::render_ui;

/// Application entry point
fn main() -> Result<(), Box<dyn Error>> {
    // Load .env file
    dotenv().ok();

    // We'll create tokio runtimes as needed for async operations

    println!("Starting Samus with Ratatui interface...");

    // Initialize config
    let app_config = config::Config::new();
    
    // Initialize TreeSitter service
    let tree_sitter_service = initialize_service(&app_config);
    
    // Setup task registry and handlers
    let mut task_registry = TaskRegistry::new();
    
    // Create filesystem implementation
    let fs_impl = std::sync::Arc::new(fs::operations::LocalFileSystem::new());
    
    // Register task handlers
    let tree_sitter_handler = std::sync::Arc::new(TreeSitterTaskHandler::new(tree_sitter_service.clone()));
    let shell_task_handler = std::sync::Arc::new(ShellTaskHandler::new());
    
    // Add handlers to registry
    task_registry.register("tree_sitter", tree_sitter_handler);
    task_registry.register("shell", shell_task_handler);
    
    // Create Arc for registry and task manager
    let task_registry = std::sync::Arc::new(task_registry);
    let task_manager = std::sync::Arc::new(TaskManager::new(fs_impl, task_registry.clone()));
    
    // Setup terminal with better error handling
    enable_raw_mode().or_else(|err| {
        eprintln!("Failed to enable raw mode: {}", err);
        Err(err)
    })?;
    
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).or_else(|err| {
        eprintln!("Failed to enter alternate screen: {}", err);
        let _ = disable_raw_mode(); // Try to clean up
        Err(err)
    })?;
    
    execute!(stdout, EnableMouseCapture).or_else(|err| {
        eprintln!("Failed to enable mouse capture: {}", err);
        let _ = disable_raw_mode(); // Try to clean up
        let _ = execute!(stdout, LeaveAlternateScreen);
        Err(err)
    })?;
    
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend).or_else(|err| {
        eprintln!("Failed to create terminal: {}", err);
        let _ = disable_raw_mode(); // Try to clean up
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        Err(err)
    })?;

    // Create app state
    let mut app = App::new();
    
    // Set task manager
    app.set_task_manager(task_manager.clone());
    
    // Initialize TreeSitter with default values
    app.init_tree_sitter(10_000_000, 5); // 10MB max file size, 5 parsers per language

    // Configure OpenRouter if API key is available
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
        if let Err(e) = app.init_llm_client(config) {
            eprintln!("Error configuring OpenRouter client: {}", e);
        }
    }

    // Main event loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal with better error handling
    if let Err(e) = disable_raw_mode() {
        eprintln!("Error disabling raw mode: {}", e);
    }
    
    if let Err(e) = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ) {
        eprintln!("Error leaving alternate screen: {}", e);
    }
    
    if let Err(e) = terminal.show_cursor() {
        eprintln!("Error showing cursor: {}", e);
    }

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        // Draw UI
        terminal.draw(|f| render_ui(f, app))?;

        // Handle events
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                // Process key event
                app.handle_key_event(key);
                
                // Check if we should quit
                if app.should_quit {
                    return Ok(());
                }
            }
        }
        
        // Handle periodic updates
        app.on_tick();
    }
}


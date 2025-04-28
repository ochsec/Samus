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
    ExecutableCommand,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dotenv::dotenv;
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};
use std::error::Error;
use std::{
    io::{self, stdout},
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;

use crate::config::McpServerConfig;
use crate::services::tree_sitter::initialize_service;
use crate::task::{TaskRegistry, TaskManager};
use crate::task::tree_sitter_task::TreeSitterTaskHandler;
use crate::task::shell_task::ShellTaskHandler;
use crate::ui::app::{App, MainViewType};
use crate::ui::tui::render_ui;

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
        Ok(_) => {}
        Err(e) => {
            println!("Failed to enable raw mode: {}", e);
            return Err(Box::new(e));
        }
    }

    // Setup terminal backend
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;

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

    // Initialize VSCode integrations
    if let Err(e) = runtime.block_on(integrations::Integrations::init()) {
        println!("Failed to initialize VSCode integrations: {}", e);
    }

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
    
    // Set task manager in app
    app.set_task_manager(task_manager.clone());

    // Add welcome message
    app.add_chat_message(
        "Welcome to Samus! Type a message or use a slash command like /help to get started."
            .to_string(),
        false,
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
            Ok(_) => app.add_chat_message(
                "OpenRouter client configured successfully with Claude 3.5 Haiku.".to_string(),
                false,
            ),
            Err(e) => {
                app.add_chat_message(format!("Error configuring OpenRouter client: {}", e), false)
            }
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

    // Cleanup terminal
    terminal.backend_mut().execute(DisableMouseCapture)?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("App error: {}", err);
    }

    Ok(())
}

/// Run the TUI interface
fn run_tui<B: Backend + crossterm::ExecutableCommand>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    // Setup mouse capture and initial view
    terminal.backend_mut().execute(EnableMouseCapture)?;

    // Set initial view
    app.set_main_view(MainViewType::ShellOutput);

    loop {
        // Render UI using the centralized render function
        terminal.clear()?;  // Clear the terminal before redrawing
        terminal.draw(|f| render_ui(f, app))?;

        // Wait for event or tick
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Check for quit command
                if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }

                // Only handle keys that should work during processing
                let is_quit_key = key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL);
                
                if !app.is_processing || is_quit_key {
                    let command = app.handle_key_event(key);
                    
                    // Ensure we still redraw for important key events
                    terminal.clear()?;
                    terminal.draw(|f| render_ui(f, app))?;
                } else {
                    // If processing, ignore other keys but don't let them pile up in the buffer
                    // This prevents keys from causing unexpected behavior when processing finishes
                    event::read()?;
                }
            }
        }

        // Check if it's time for a tick
        if last_tick.elapsed() >= tick_rate {
            let was_processing = app.is_processing;
            app.on_tick();
            
            // Force a redraw if processing state changed
            if was_processing != app.is_processing {
                terminal.clear()?;
                terminal.draw(|f| render_ui(f, app))?;
            }
            
            last_tick = Instant::now();
        }

        // Check if we should exit
        if app.should_quit {
            break;
        }
    }

    // Disable mouse support before exit
    terminal.backend_mut().execute(DisableMouseCapture)?;

    Ok(())
}

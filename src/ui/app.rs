use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::mcp::client::OpenRouterClient;
use crate::services::tree_sitter::TreeSitterService;
use crate::task::TaskManagerTrait;
use crate::ui::input::{InputCommand, InputHandler, InputMode};
use crate::ui::output::OutputManager;

/// Maximum number of chat messages to keep in history
const MAX_CHAT_HISTORY: usize = 100;

/// Represents different view types for the main area
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MainViewType {
    FileTree,
    GitDiff,
    ShellOutput,
    LlmResponse,
    Search,
    CodeOutline,
}

/// Represents a chat message with metadata
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub content: String,
    pub is_user: bool,
    pub timestamp: Instant,
}

/// Represents a code symbol for display
#[derive(Debug, Clone)]
pub struct DisplaySymbol {
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub path: String,
}

/// Represents the main application state and logic
pub struct App {
    // Core state
    pub input_handler: InputHandler,
    pub output_manager: OutputManager,

    // Input state
    pub input_text: String,
    pub cursor_position: usize,
    pub input_mode: InputMode,
    pub command_history: VecDeque<String>,
    pub history_index: Option<usize>,

    // Chat state
    pub chat_messages: VecDeque<ChatMessage>,
    pub llm_client: Option<OpenRouterClient>,
    pub is_processing: bool,

    // View state
    pub current_main_view: MainViewType,
    pub should_quit: bool,
    pub displaying_completion: bool, // Whether currently displaying a completion

    // Code analysis state
    pub tree_sitter_service: Option<Arc<TreeSitterService>>,
    pub current_file_symbols: Vec<DisplaySymbol>,
    pub current_file_path: Option<String>,

    // Task management
    pub task_manager: Option<Arc<crate::task::TaskManager>>,

    // Application timing
    pub tick_rate: Duration,
    pub last_tick: Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            input_handler: InputHandler::new(),
            output_manager: OutputManager::new(),

            input_text: String::new(),
            cursor_position: 0,
            input_mode: InputMode::Normal,
            command_history: VecDeque::with_capacity(50),
            history_index: None,

            chat_messages: VecDeque::with_capacity(MAX_CHAT_HISTORY),
            llm_client: None,
            is_processing: false,

            current_main_view: MainViewType::ShellOutput,
            should_quit: false,
            displaying_completion: false,

            tree_sitter_service: None,
            current_file_symbols: Vec::new(),
            current_file_path: None,
            
            task_manager: None,

            tick_rate: Duration::from_millis(250),
            last_tick: Instant::now(),
        }
    }
    
    /// Show the input area
    pub fn show_input_area(&mut self) {
        self.displaying_completion = false;
    }
    
    /// Hide the input area
    pub fn hide_input_area(&mut self) {
        self.displaying_completion = true;
    }
    
    /// Set the task manager
    pub fn set_task_manager(&mut self, task_manager: Arc<crate::task::TaskManager>) {
        self.task_manager = Some(task_manager);
    }

    /// Initialize TreeSitter service
    pub fn init_tree_sitter(&mut self, max_file_size: usize, max_parsers_per_lang: usize) {
        self.tree_sitter_service = Some(Arc::new(TreeSitterService::new(
            max_file_size,
            max_parsers_per_lang,
        )));
    }

    /// Initialize OpenRouter client with provided config
    pub fn init_llm_client(
        &mut self,
        config: crate::config::McpServerConfig,
    ) -> Result<(), crate::error::TaskError> {
        // Initialize with Claude 3.5 Haiku as the default model
        let client = OpenRouterClient::new(config, "anthropic/claude-3-haiku".to_string())?;
        self.llm_client = Some(client);
        Ok(())
    }

    /// Add a message to the chat history
    pub fn add_chat_message(&mut self, content: String, is_user: bool) {
        if self.chat_messages.len() >= MAX_CHAT_HISTORY {
            self.chat_messages.pop_front();
        }

        self.chat_messages.push_back(ChatMessage {
            content,
            is_user,
            timestamp: Instant::now(),
        });
    }

    /// Add a command to history
    pub fn add_to_history(&mut self, command: String) {
        if command.is_empty()
            || (self
                .command_history
                .front()
                .map_or(false, |c| c == &command))
        {
            return;
        }

        if self.command_history.len() >= 50 {
            self.command_history.pop_back();
        }

        self.command_history.push_front(command);
        self.history_index = None;
    }

    /// Process input text
    pub fn process_input(&mut self) {
        // Take the input text and ensure the cursor position is reset
        let input = std::mem::take(&mut self.input_text);
        self.cursor_position = 0; // Reset cursor position
        
        if input.is_empty() {
            return;
        }

        self.add_to_history(input.clone());
        self.add_chat_message(input.clone(), true);
        
        // Set current view to ShellOutput and hide input area - this makes output fill the screen
        self.current_main_view = MainViewType::ShellOutput;
        self.displaying_completion = true;

        // Process command based on prefix
        if input.starts_with('/') {
            // Slash command
            self.process_slash_command(&input[1..]);
        } else if input.starts_with('!') {
            // Bash command
            self.process_bash_command(&input[1..]);
        } else if input.starts_with('@') {
            // File reference
            self.process_file_reference(&input[1..]);
        } else {
            // Normal input - send to LLM
            self.send_to_llm(input);
        }
    }

    /// Send user input to LLM and handle the response
    pub fn send_to_llm(&mut self, prompt: String) {
        // Mark as processing
        self.is_processing = true;

        // Check if client is initialized
        if let Some(client) = self.llm_client.clone() {
            // Create a message indicating we're waiting for a response
            self.add_chat_message("Thinking...".to_string(), false);

            // Use a thread to handle the async request without blocking the UI
            let (tx, rx) = std::sync::mpsc::channel();

            // Clone necessary values for the thread
            let prompt_clone = prompt.clone();

            // Spawn a thread to handle the async request
            std::thread::spawn(move || {
                // Create a tokio runtime for async operations
                let rt = tokio::runtime::Runtime::new().unwrap();

                // Execute the chat request
                let result = rt.block_on(async { client.chat(prompt_clone).await });

                // Send the result back to the main thread
                tx.send(result).unwrap();
            });

            // Store the receiver for later checking in on_tick
            self.output_manager.store_receiver(rx);
        } else {
            // No client configured
            self.add_chat_message(
                "Error: LLM client not initialized. Use /config to set up OpenRouter.".to_string(),
                false,
            );
            self.is_processing = false;
            self.displaying_completion = false;
        }
    }

    /// Process LLM response when available
    pub fn check_llm_response(&mut self) {
        if let Some(result) = self.output_manager.check_llm_response() {
            // Remove the "Thinking..." message if it exists
            // Find the last "Thinking..." message from the assistant
            if let Some(thinking_idx) = self.chat_messages.iter().position(|msg| 
                !msg.is_user && msg.content == "Thinking..."
            ) {
                // Remove it safely
                self.chat_messages.remove(thinking_idx);
            }

            match result {
                Ok(content) => {
                    // Add the actual response
                    self.add_chat_message(content, false);
                }
                Err(e) => {
                    // Add error message
                    self.add_chat_message(format!("Error: {}", e), false);
                }
            }

            // No need to reset scroll position as we're using terminal scrollback
            
            // Mark as no longer processing but keep the completion in full-screen mode
            // The user can type to automatically exit fullscreen mode
            self.is_processing = false;
            self.displaying_completion = true;
        }
    }

    /// Process slash commands
    fn process_slash_command(&mut self, command: &str) {
        let response = match command.trim() {
            "help" => {
                "Available commands: /help, /quit, /search, /diff, /model, /outline, /ls, /dir".to_string()
            }
            "quit" => {
                self.should_quit = true;
                "Exiting application...".to_string()
            }
            "search" => {
                self.current_main_view = MainViewType::Search;
                "Switched to search view".to_string()
            }
            cmd if cmd.starts_with("diff") => {
                self.current_main_view = MainViewType::GitDiff;
                "Showing diff view".to_string()
            }
            cmd if cmd.starts_with("model") => self.set_model_command(cmd).to_string(),
            cmd if cmd.starts_with("config") => self.configure_openrouter_command(cmd).to_string(),
            cmd if cmd.starts_with("outline") => {
                self.current_main_view = MainViewType::CodeOutline;
                self.show_code_outline(cmd)
            }
            cmd if cmd.starts_with("ls") || cmd.starts_with("dir") => {
                self.list_directory_command(cmd)
            }
            _ => "Unknown command. Try /help for a list of commands.".to_string(),
        };

        self.add_chat_message(response, false);
    }

    /// List directory contents using the shell task handler
    fn list_directory_command(&mut self, cmd: &str) -> String {
        // Parse path from command (format: /ls [path] or /dir [path])
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let path = if parts.len() >= 2 {
            parts[1].to_string()
        } else {
            ".".to_string()  // Current directory by default
        };
        
        // Parse recursive flag
        let recursive = parts.len() >= 3 && parts[2].eq_ignore_ascii_case("-r");
        
        // Check if task manager is available
        if let Some(task_manager) = &self.task_manager {
            use crate::task::Task;
            use serde_json::json;
            
            // Create a shell task to list the directory
            let task = Task::new("shell", json!({
                "type": "list_directory",
                "path": path,
                "recursive": recursive
            }));
            
            // Mark as processing
            self.is_processing = true;
            
            // Clone task manager for thread
            let task_manager_clone = task_manager.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            
            // Spawn a thread to handle the async execution
            std::thread::spawn(move || {
                // Create a tokio runtime for async operations
                let rt = tokio::runtime::Runtime::new().unwrap();
                
                // Execute the task
                let result = rt.block_on(async { task_manager_clone.execute_task(task).await });
                
                // Send result back to main thread
                tx.send(result).unwrap();
            });
            
            // Store receiver for later checking
            self.output_manager.store_shell_receiver(rx);
            
            // Return intermediate message
            format!("Listing {}directory contents for: {}", 
                if recursive { "recursive " } else { "" }, 
                path)
        } else {
            // No task manager available
            "Error: Task manager not initialized.".to_string()
        }
    }
    
    /// Show code outline for a file
    fn show_code_outline(&mut self, cmd: &str) -> String {
        // Parse file path if provided
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let file_path = if parts.len() >= 2 {
            parts[1].to_string()
        } else if let Some(path) = &self.current_file_path {
            path.clone()
        } else {
            return "Usage: /outline <file_path>".to_string();
        };

        // Check if TreeSitter service is initialized
        let service = match &self.tree_sitter_service {
            Some(service) => service.clone(),
            None => return "Error: TreeSitter service not initialized.".to_string(),
        };

        // Try to read the file
        match std::fs::read_to_string(&file_path) {
            Ok(content) => {
                // Try to parse the file and extract symbols
                let path = Path::new(&file_path);
                match service.find_symbols(path, &content) {
                    Ok(symbols) => {
                        // Convert symbols to display symbols
                        self.current_file_symbols = symbols
                            .into_iter()
                            .map(|s| DisplaySymbol {
                                name: s.name,
                                kind: format!("{:?}", s.kind),
                                line: s.start_line,
                                path: file_path.clone(),
                            })
                            .collect();

                        self.current_file_path = Some(file_path.clone());
                        format!(
                            "Found {} symbols in {}",
                            self.current_file_symbols.len(),
                            file_path
                        )
                    }
                    Err(e) => {
                        format!("Error parsing file: {}", e)
                    }
                }
            }
            Err(e) => {
                format!("Error reading file {}: {}", file_path, e)
            }
        }
    }

    /// Handle model setting command
    fn set_model_command(&mut self, cmd: &str) -> String {
        // Check if client exists
        if self.llm_client.is_none() {
            return "Error: LLM client not initialized. Use /config to set up OpenRouter."
                .to_string();
        }

        // Parse model name (expecting format: /model <model_name>)
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.len() < 2 {
            return "Usage: /model <model_name> - Current model is Claude 3.5 Haiku".to_string();
        }

        // Set the model
        let model_arg = parts[1];
        let model_name = match model_arg {
            "haiku" => "anthropic/claude-3-haiku",
            "opus" => "anthropic/claude-3-opus",
            "sonnet" => "anthropic/claude-3-sonnet",
            other => other, // Use exact name if provided
        };

        // Update the client
        if let Some(client) = &mut self.llm_client {
            client.set_model(model_name.to_string());
            return "Model updated successfully".to_string();
        }

        "Error updating model".to_string()
    }

    /// Handle OpenRouter configuration
    fn configure_openrouter_command(&mut self, cmd: &str) -> String {
        // Parse config (expecting format: /config <api_key>)
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.len() < 2 {
            return "Usage: /config <api_key>".to_string();
        }

        // Create config and initialize client
        let api_key = parts[1];
        let config = crate::config::McpServerConfig {
            id: "openrouter".to_string(),
            name: "OpenRouter".to_string(),
            url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
            api_key: Some(api_key.to_string()),
            enabled: true,
        };

        // Initialize client
        match self.init_llm_client(config) {
            Ok(_) => "OpenRouter client configured successfully with Claude 3.5 Haiku".to_string(),
            Err(_) => "Error configuring OpenRouter client".to_string(),
        }
    }

    /// Process bash commands
    fn process_bash_command(&mut self, command: &str) {
        self.current_main_view = MainViewType::ShellOutput;
        self.add_chat_message(format!("Executing bash command: {}", command), false);
        
        // Check if task manager is available
        if let Some(task_manager) = &self.task_manager {
            use crate::task::Task;
            use serde_json::json;
            
            // Create a shell task to execute the command
            let task = Task::new("shell", json!({
                "type": "execute",
                "command": command,
                "capture_stderr": true
            }));
            
            // Mark as processing
            self.is_processing = true;
            
            // Clone task manager for thread
            let task_manager_clone = task_manager.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            
            // Spawn a thread to handle the async execution
            std::thread::spawn(move || {
                // Create a tokio runtime for async operations
                let rt = tokio::runtime::Runtime::new().unwrap();
                
                // Execute the task
                let result = rt.block_on(async { task_manager_clone.execute_task(task).await });
                
                // Send result back to main thread
                tx.send(result).unwrap();
            });
            
            // Store receiver for later checking
            self.output_manager.store_shell_receiver(rx);
        } else {
            // No task manager available
            self.add_chat_message("Error: Task manager not initialized.".to_string(), false);
            self.is_processing = false;
        }
    }

    /// Process file references
    fn process_file_reference(&mut self, path: &str) {
        self.current_main_view = MainViewType::FileTree;
        self.add_chat_message(format!("Referencing file: {}", path), false);

        // Try to parse the file with TreeSitter if the service is available
        if let Some(service) = &self.tree_sitter_service {
            // Try to read and parse the file
            self.current_file_path = Some(path.to_string());
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    let path_obj = Path::new(path);
                    match service.find_symbols(path_obj, &content) {
                        Ok(symbols) => {
                            // Convert symbols to display symbols
                            self.current_file_symbols = symbols
                                .into_iter()
                                .map(|s| DisplaySymbol {
                                    name: s.name,
                                    kind: format!("{:?}", s.kind),
                                    line: s.start_line,
                                    path: path.to_string(),
                                })
                                .collect();

                            self.add_chat_message(
                                format!(
                                    "File parsed successfully. Found {} symbols.",
                                    self.current_file_symbols.len()
                                ),
                                false,
                            );
                        }
                        Err(e) => {
                            self.add_chat_message(format!("Error parsing file: {}", e), false);
                        }
                    }
                }
                Err(e) => {
                    self.add_chat_message(format!("Error reading file: {}", e), false);
                }
            }
        }
    }

    /// Set the main view type
    pub fn set_main_view(&mut self, view_type: MainViewType) {
        self.current_main_view = view_type;
    }

    /// Handle key events
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<InputCommand> {
        // Reset cursor position if it's somehow outside bounds
        // This is a safety check to prevent string boundary errors
        self.cursor_position = self.cursor_position.min(self.input_text.len());
        
        // First, check for custom key bindings from the input handler
        let command = self.input_handler.handle_key_event(key);
        if command != InputCommand::None {
            // Process scrolling commands
            // We no longer handle scrolling commands
            return Some(command);
        }
        
        // We're no longer doing custom scroll handling with arrow keys and page up/down
    // Instead we're relying on the terminal's built-in scrollback buffer
    // Just handle Escape key to toggle input visibility
        
        match key {
            // Handle Escape key to toggle between full-screen output and input mode
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Toggle between full-screen output and showing the input area
                self.displaying_completion = !self.displaying_completion;
                return Some(InputCommand::None);
            }
            
            // Quit application with Ctrl+Q
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.should_quit = true;
                return Some(InputCommand::Quit);
            }

            // Handle Enter to submit input or create a new line
            KeyEvent {
                code: KeyCode::Enter,
                modifiers,
                ..
            } => {
                // Check if we're in a code block
                if self.is_in_code_block() {
                    // Inside code block, add a new line
                    self.input_text.insert(self.cursor_position, '\n');
                    self.cursor_position += 1;
                    return Some(InputCommand::None);
                } else if modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Enter always adds a new line
                    self.input_text.insert(self.cursor_position, '\n');
                    self.cursor_position += 1;
                    return Some(InputCommand::None);
                } else {
                    // Normal Enter submits the input
                    self.process_input();
                    // Reset cursor after input processing
                    self.cursor_position = 0;
                    return Some(InputCommand::None);
                }
            }

            // Handle Backspace for input area
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If we're in full-screen completion mode, exit it
                if self.displaying_completion {
                    self.displaying_completion = false;
                }
                
                if self.cursor_position > 0 {
                    // Ensure we're at a char boundary before removing
                    let new_pos = self.find_prev_char_boundary(self.cursor_position);
                    self.input_text.remove(new_pos);
                    self.cursor_position = new_pos;
                }
                return Some(InputCommand::None);
            }

            // Handle Delete for input area
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If we're in full-screen completion mode, exit it
                if self.displaying_completion {
                    self.displaying_completion = false;
                }
                
                if self.cursor_position < self.input_text.len() {
                    self.input_text.remove(self.cursor_position);
                }
                return Some(InputCommand::None);
            }

            // Move cursor left
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If we're in full-screen completion mode, exit it
                if self.displaying_completion {
                    self.displaying_completion = false;
                }
                
                if self.cursor_position > 0 {
                    // Find previous valid char boundary
                    self.cursor_position = self.find_prev_char_boundary(self.cursor_position);
                }
                return Some(InputCommand::None);
            }

            // Move cursor right
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If we're in full-screen completion mode, exit it
                if self.displaying_completion {
                    self.displaying_completion = false;
                }
                
                if self.cursor_position < self.input_text.len() {
                    // Find next valid char boundary
                    self.cursor_position = self.find_next_char_boundary(self.cursor_position);
                }
                return Some(InputCommand::None);
            }

            // Handle history navigation up
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If we're in full-screen completion mode, exit it
                if self.displaying_completion {
                    self.displaying_completion = false;
                }
                
                self.navigate_history_up();
                return Some(InputCommand::None);
            }

            // Handle history navigation down
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If we're in full-screen completion mode, exit it
                if self.displaying_completion {
                    self.displaying_completion = false;
                }
                
                self.navigate_history_down();
                return Some(InputCommand::None);
            }

            // Handle normal key input
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                ..
            } => {
                // If we're in full-screen completion mode, exit it when user starts typing
                if self.displaying_completion {
                    self.displaying_completion = false;
                }
                
                // Insert at a valid UTF-8 boundary
                if self.cursor_position <= self.input_text.len() {
                    self.input_text.insert(self.cursor_position, c);
                    self.cursor_position += c.len_utf8();
                } else {
                    // Safety fallback if cursor is somehow out of bounds
                    self.input_text.push(c);
                    self.cursor_position = self.input_text.len();
                }
                return Some(InputCommand::None);
            }

            // Default case - if we get here, no specific handler matched
            _ => {
                return Some(InputCommand::None);
            }
        }
    }
    
    /// Helper method to find the previous valid UTF-8 character boundary
    fn find_prev_char_boundary(&self, from: usize) -> usize {
        let mut pos = from.saturating_sub(1);
        while pos > 0 && !self.input_text.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }
    
    /// Helper method to find the next valid UTF-8 character boundary
    fn find_next_char_boundary(&self, from: usize) -> usize {
        let mut pos = from + 1;
        while pos < self.input_text.len() && !self.input_text.is_char_boundary(pos) {
            pos += 1;
        }
        pos.min(self.input_text.len())
    }

    /// Navigate command history upward
    fn navigate_history_up(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let next_index = match self.history_index {
            None => Some(0),
            Some(i) if i + 1 < self.command_history.len() => Some(i + 1),
            _ => return,
        };

        self.history_index = next_index;
        if let Some(idx) = next_index {
            // Replace input text safely
            self.input_text = self.command_history[idx].clone();
            
            // Set cursor to the end, ensuring it's at a valid char boundary
            let text_len = self.input_text.len();
            self.cursor_position = if text_len > 0 {
                if self.input_text.is_char_boundary(text_len) {
                    text_len
                } else {
                    // Find the last valid boundary if text_len isn't one
                    let mut pos = text_len - 1;
                    while pos > 0 && !self.input_text.is_char_boundary(pos) {
                        pos -= 1;
                    }
                    pos
                }
            } else {
                0
            };
        }
    }

    /// Navigate command history downward
    fn navigate_history_down(&mut self) {
        if let Some(idx) = self.history_index {
            if idx == 0 {
                // At most recent history item, clear input
                self.history_index = None;
                self.input_text.clear();
                self.cursor_position = 0;
            } else {
                // Go to more recent history item
                self.history_index = Some(idx - 1);
                self.input_text = self.command_history[idx - 1].clone();
                
                // Set cursor to the end, ensuring it's at a valid char boundary
                let text_len = self.input_text.len();
                self.cursor_position = if text_len > 0 {
                    if self.input_text.is_char_boundary(text_len) {
                        text_len
                    } else {
                        // Find the last valid boundary if text_len isn't one
                        let mut pos = text_len - 1;
                        while pos > 0 && !self.input_text.is_char_boundary(pos) {
                            pos -= 1;
                        }
                        pos
                    }
                } else {
                    0
                };
            }
        }
    }

    /// Update app state on tick
    pub fn on_tick(&mut self) {
        self.last_tick = Instant::now();

        // Check for LLM responses and shell command results
        if self.is_processing {
            self.check_llm_response();
            self.check_shell_result();
        }
    }
    
    /// Check for shell command results
    fn check_shell_result(&mut self) {
        if let Some(result) = self.output_manager.check_shell_result() {
            // Find and remove any "Executing..." or similar pending message
            // This follows the same pattern as check_llm_response for consistency
            if let Some(pending_idx) = self.chat_messages.iter().position(|msg| 
                !msg.is_user && (msg.content.starts_with("Executing bash command:") || 
                                msg.content.starts_with("Listing"))
            ) {
                // Only remove if it's the most recent message from the assistant
                if self.chat_messages.iter().skip(pending_idx + 1).all(|msg| msg.is_user) {
                    self.chat_messages.remove(pending_idx);
                }
            }

            match result {
                Ok(task_result) => {
                    // Convert task result to string based on its type
                    let result_str = match task_result {
                        crate::task::TaskResult::Text(text) => text,
                        crate::task::TaskResult::Json(json) => format!("{}", json),
                        crate::task::TaskResult::Binary(bytes) => format!("[Binary data: {} bytes]", bytes.len()),
                    };
                    
                    // Add the result to chat messages and update view
                    self.add_chat_message(result_str, false);
                    
                    // Switch to shell output view to make results more visible
                    if self.current_main_view != MainViewType::ShellOutput {
                        self.current_main_view = MainViewType::ShellOutput;
                    }
                }
                Err(e) => {
                    // Add error message
                    self.add_chat_message(format!("Error executing command: {}", e), false);
                }
            }
            
            // No need to reset scroll position as we're using terminal scrollback
            
            // Mark as no longer processing but keep the completion in full-screen mode
            // The user can type to automatically exit fullscreen mode
            self.is_processing = false;
            self.displaying_completion = true;
        }
    }

    /// Check if cursor is within a code block
    fn is_in_code_block(&self) -> bool {
        // Find triple backticks before and after cursor position
        let text_before_cursor = &self.input_text[..self.cursor_position];
        let text_after_cursor = &self.input_text[self.cursor_position..];

        // Count backtick blocks before cursor
        let backtick_blocks_before = text_before_cursor.matches("```").count();

        // Count backtick blocks after cursor
        let backtick_blocks_after = text_after_cursor.matches("```").count();

        // If there's an odd number of backtick blocks before cursor and at least one after,
        // we're inside a code block
        backtick_blocks_before % 2 == 1 && backtick_blocks_after > 0
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

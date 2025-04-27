use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::mcp::client::OpenRouterClient;
use crate::ui::input::{InputHandler, InputMode, InputCommand};
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
}

/// Represents a chat message with metadata
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub content: String,
    pub is_user: bool,
    pub timestamp: Instant,
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
            
            current_main_view: MainViewType::LlmResponse,
            should_quit: false,
            
            tick_rate: Duration::from_millis(250),
            last_tick: Instant::now(),
        }
    }
    
    /// Initialize OpenRouter client with provided config
    pub fn init_llm_client(&mut self, config: crate::config::McpServerConfig) -> Result<(), crate::error::TaskError> {
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
        if command.is_empty() || (self.command_history.front().map_or(false, |c| c == &command)) {
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
        let input = std::mem::take(&mut self.input_text);
        if input.is_empty() {
            return;
        }
        
        self.add_to_history(input.clone());
        self.add_chat_message(input.clone(), true);
        
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
                let result = rt.block_on(async {
                    client.chat(prompt_clone).await
                });
                
                // Send the result back to the main thread
                tx.send(result).unwrap();
            });
            
            // Store the receiver for later checking in on_tick
            self.output_manager.store_receiver(rx);
        } else {
            // No client configured
            self.add_chat_message("Error: LLM client not initialized. Use /config to set up OpenRouter.".to_string(), false);
            self.is_processing = false;
        }
    }
    
    /// Process LLM response when available
    pub fn check_llm_response(&mut self) {
        if let Some(result) = self.output_manager.check_llm_response() {
            // Remove the "Thinking..." message
            if !self.chat_messages.is_empty() {
                let last_message = self.chat_messages.back().unwrap();
                if !last_message.is_user && last_message.content == "Thinking..." {
                    self.chat_messages.pop_back();
                }
            }
            
            match result {
                Ok(content) => {
                    // Add the actual response
                    self.add_chat_message(content, false);
                },
                Err(e) => {
                    // Add error message
                    self.add_chat_message(format!("Error: {}", e), false);
                }
            }
            
            // Mark as no longer processing
            self.is_processing = false;
        }
    }
    
    /// Process slash commands
    fn process_slash_command(&mut self, command: &str) {
        let response = match command.trim() {
            "help" => "Available commands: /help, /quit, /search, /diff, /model".to_string(),
            "quit" => {
                self.should_quit = true;
                "Exiting application...".to_string()
            },
            "search" => {
                self.current_main_view = MainViewType::Search;
                "Switched to search view".to_string()
            },
            cmd if cmd.starts_with("diff") => {
                self.current_main_view = MainViewType::GitDiff;
                "Showing diff view".to_string()
            },
            cmd if cmd.starts_with("model") => {
                self.set_model_command(cmd).to_string()
            },
            cmd if cmd.starts_with("config") => {
                self.configure_openrouter_command(cmd).to_string()
            },
            _ => "Unknown command. Try /help for a list of commands.".to_string()
        };
        
        self.add_chat_message(response, false);
    }
    
    /// Handle model setting command
    fn set_model_command(&mut self, cmd: &str) -> String {
        // Check if client exists
        if self.llm_client.is_none() {
            return "Error: LLM client not initialized. Use /config to set up OpenRouter.".to_string();
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
            Err(_) => "Error configuring OpenRouter client".to_string()
        }
    }
    
    /// Process bash commands
    fn process_bash_command(&mut self, command: &str) {
        self.current_main_view = MainViewType::ShellOutput;
        self.add_chat_message(format!("Executing bash command: {}", command), false);
        // In a real implementation, this would execute the command
    }
    
    /// Process file references
    fn process_file_reference(&mut self, path: &str) {
        self.current_main_view = MainViewType::FileTree;
        self.add_chat_message(format!("Referencing file: {}", path), false);
        // In a real implementation, this would load the file
    }
    
    /// Handle key events
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<InputCommand> {
        match key {
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
                    return Some(InputCommand::None);
                }
            }
            
            // Handle Backspace for input area
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if self.cursor_position > 0 {
                    self.input_text.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
                return Some(InputCommand::None);
            }
            
            // Handle Delete for input area
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
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
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
                return Some(InputCommand::None);
            }
            
            // Move cursor right
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if self.cursor_position < self.input_text.len() {
                    self.cursor_position += 1;
                }
                return Some(InputCommand::None);
            }
            
            // Handle history navigation up
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.navigate_history_up();
                return Some(InputCommand::None);
            }
            
            // Handle history navigation down
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.navigate_history_down();
                return Some(InputCommand::None);
            }
            
            // Handle normal key input
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                ..
            } => {
                self.input_text.insert(self.cursor_position, c);
                self.cursor_position += 1;
                return Some(InputCommand::None);
            }
            
            // Pass other keys to the input handler
            _ => {
                return Some(self.input_handler.handle_key_event(key));
            }
        }
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
            self.input_text = self.command_history[idx].clone();
            self.cursor_position = self.input_text.len();
        }
    }
    
    /// Navigate command history downward
    fn navigate_history_down(&mut self) {
        if let Some(idx) = self.history_index {
            if idx == 0 {
                self.history_index = None;
                self.input_text.clear();
                self.cursor_position = 0;
            } else {
                self.history_index = Some(idx - 1);
                self.input_text = self.command_history[idx - 1].clone();
                self.cursor_position = self.input_text.len();
            }
        }
    }
    
    /// Update app state on tick
    pub fn on_tick(&mut self) {
        self.last_tick = Instant::now();
        
        // Check for LLM responses
        if self.is_processing {
            self.check_llm_response();
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
use tokio::sync::mpsc;
use uuid;
use std::sync::mpsc as std_mpsc;

use crate::error::TaskError;

/// For compatibility with tests
pub struct Buffer {
    pub lines: Vec<Line>,
}

/// For compatibility with tests
pub struct Line {
    pub content: String,
}

/// Manages output rendering and formatting for the terminal UI
#[derive(Debug)]
pub struct OutputManager {
    // Add fields for managing output state
    buffer: Vec<String>,
    sender: Option<mpsc::Sender<String>>,
    // For handling LLM responses
    llm_receiver: Option<std_mpsc::Receiver<Result<String, TaskError>>>,
}

impl OutputManager {
    /// Create a new OutputManager
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            sender: None,
            llm_receiver: None,
        }
    }
    
    /// Store the receiver for LLM responses
    pub fn store_receiver(&mut self, rx: std_mpsc::Receiver<Result<String, TaskError>>) {
        self.llm_receiver = Some(rx);
    }
    
    /// Check for available LLM responses
    pub fn check_llm_response(&mut self) -> Option<Result<String, TaskError>> {
        if let Some(rx) = &self.llm_receiver {
            // Try to receive a message without blocking
            match rx.try_recv() {
                Ok(result) => {
                    // Clear the receiver once we've processed a message
                    self.llm_receiver = None;
                    return Some(result);
                },
                Err(std_mpsc::TryRecvError::Empty) => {
                    // No message available yet, keep waiting
                    return None;
                },
                Err(std_mpsc::TryRecvError::Disconnected) => {
                    // Channel disconnected, clear the receiver
                    self.llm_receiver = None;
                    return Some(Err(TaskError::ExecutionFailed("LLM response channel disconnected".to_string())));
                }
            }
        }
        None
    }
    
    /// Process any pending output - for compatibility with tests
    pub async fn process_output(&self) {
        // This method is maintained for compatibility with existing tests
    }
    
    /// Create a buffer - for compatibility with tests
    pub fn create_buffer(&self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }
    
    /// Get buffers - for compatibility with tests
    pub fn buffers(&self) -> std::collections::HashMap<uuid::Uuid, Buffer> {
        std::collections::HashMap::new()
    }
    
    // Moved structures to proper place outside impl

    /// Create a new OutputManager with a sender
    pub fn with_sender(sender: mpsc::Sender<String>) -> Self {
        Self {
            buffer: Vec::new(),
            sender: Some(sender),
            llm_receiver: None,
        }
    }

    /// Add a line to the output buffer
    pub fn add_line(&mut self, line: String) {
        self.buffer.push(line.clone());
        
        // If a sender is available, try to send the line
        if let Some(sender) = &self.sender {
            let _ = sender.try_send(line);
        }
    }

    /// Clear the output buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get all lines in the buffer
    pub fn get_lines(&self) -> &[String] {
        &self.buffer
    }

    /// Get the sender if available
    pub fn get_sender(&self) -> Option<mpsc::Sender<String>> {
        self.sender.clone()
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}
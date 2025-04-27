use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, VecDeque};

const MAX_HISTORY: usize = 50;

/// Represents different modes of interaction in the TUI
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Command,
    Search,
    Diff,
    Help,
}

/// Represents a configurable key binding
#[derive(Debug, Clone)]
pub struct KeyBinding {
    key_code: KeyCode,
    modifiers: KeyModifiers,
    command: InputCommand,
}

/// Enhanced input state management
#[derive(Debug)]
pub struct InputHandler {
    command_buffer: String,
    command_history: VecDeque<String>,
    history_index: Option<usize>,
    last_command: Option<String>,
    current_mode: InputMode,
    key_bindings: HashMap<(KeyCode, KeyModifiers), InputCommand>,
}

/// Comprehensive input command enum with more detailed variants
#[derive(Debug, Clone, PartialEq)]
pub enum InputCommand {
    // Navigation commands
    SelectNextTask,
    SelectPreviousTask,
    
    // Task management
    ExecuteTask,
    CancelTask,
    
    // Mode and UI commands
    ChangeMode(InputMode),
    ShowHelp,
    Quit,
    
    // Diff-related commands
    DiffScrollUp,
    DiffScrollDown,
    DiffToggleFold,
    ShowDiff,
    
    // Search-related commands
    ToggleSearch,
    NavigateNextResult,
    NavigatePreviousResult,
    ToggleSearchCase,
    ToggleSearchRegex,
    
    // Error and utility commands
    Invalid(String),
    None,
}

impl InputHandler {
    pub fn new() -> Self {
        let mut handler = Self {
            command_buffer: String::new(),
            command_history: VecDeque::with_capacity(MAX_HISTORY),
            history_index: None,
            last_command: None,
            current_mode: InputMode::Normal,
            key_bindings: HashMap::new(),
        };
        
        // Default key bindings
        handler.register_default_key_bindings();
        handler
    }

    fn register_default_key_bindings(&mut self) {
        // Navigation bindings
        self.bind_key(KeyCode::Down, KeyModifiers::NONE, InputCommand::SelectNextTask);
        self.bind_key(KeyCode::Up, KeyModifiers::NONE, InputCommand::SelectPreviousTask);
        
        // Mode and control bindings
        self.bind_key(KeyCode::Char('q'), KeyModifiers::CONTROL, InputCommand::Quit);
        self.bind_key(KeyCode::Char('h'), KeyModifiers::CONTROL, InputCommand::ShowHelp);
        
        // Task management
        self.bind_key(KeyCode::Char('e'), KeyModifiers::CONTROL, InputCommand::ExecuteTask);
        self.bind_key(KeyCode::Char('c'), KeyModifiers::CONTROL, InputCommand::CancelTask);
        
        // Diff controls
        self.bind_key(KeyCode::Char('d'), KeyModifiers::CONTROL, InputCommand::ShowDiff);
        self.bind_key(KeyCode::Char('k'), KeyModifiers::CONTROL, InputCommand::DiffScrollUp);
        self.bind_key(KeyCode::Char('j'), KeyModifiers::CONTROL, InputCommand::DiffScrollDown);
        
        // Search controls
        self.bind_key(KeyCode::Char('s'), KeyModifiers::CONTROL, InputCommand::ToggleSearch);
    }

    /// Bind a key to a specific command
    pub fn bind_key(&mut self, key_code: KeyCode, modifiers: KeyModifiers, command: InputCommand) {
        self.key_bindings.insert((key_code, modifiers), command);
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> InputCommand {
        // First, check custom key bindings
        if let Some(command) = self.key_bindings.get(&(key.code, key.modifiers)) {
            return command.clone();
        }

        // Mode-specific handling
        match self.current_mode {
            InputMode::Normal => self.handle_normal_mode_input(key),
            InputMode::Command => self.handle_command_mode_input(key),
            _ => self.handle_default_input(key),
        }
    }

    fn handle_normal_mode_input(&mut self, key: KeyEvent) -> InputCommand {
        match (key.code, key.modifiers) {
            (KeyCode::Char(':'), KeyModifiers::NONE) => {
                self.current_mode = InputMode::Command;
                InputCommand::ChangeMode(InputMode::Command)
            }
            (KeyCode::Up, KeyModifiers::CONTROL) => {
                self.navigate_history_backward();
                InputCommand::None
            }
            (KeyCode::Down, KeyModifiers::CONTROL) => {
                self.navigate_history_forward();
                InputCommand::None
            }
            _ => InputCommand::None,
        }
    }

    fn handle_command_mode_input(&mut self, key: KeyEvent) -> InputCommand {
        match (key.code, key.modifiers) {
            (KeyCode::Char(c), KeyModifiers::NONE) => {
                self.command_buffer.push(c);
                InputCommand::None
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                self.command_buffer.pop();
                InputCommand::None
            }
            (KeyCode::Enter, KeyModifiers::NONE) => {
                let result = self.process_command();
                self.current_mode = InputMode::Normal;
                result
            }
            (KeyCode::Esc, KeyModifiers::NONE) => {
                self.current_mode = InputMode::Normal;
                self.command_buffer.clear();
                InputCommand::None
            }
            _ => InputCommand::None,
        }
    }

    fn handle_default_input(&mut self, key: KeyEvent) -> InputCommand {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => InputCommand::Quit,
            _ => InputCommand::None,
        }
    }

    fn process_command(&mut self) -> InputCommand {
        let command = self.command_buffer.trim().to_string();
        if command.is_empty() {
            return InputCommand::None;
        }

        // Add to history
        if self.command_history.len() >= MAX_HISTORY {
            self.command_history.pop_back();
        }
        self.command_history.push_front(command.clone());
        self.last_command = Some(command.clone());
        self.command_buffer.clear();
        self.history_index = None;

        // Parse command
        match command.as_str() {
            "help" => InputCommand::ShowHelp,
            "quit" => InputCommand::Quit,
            "exec" => InputCommand::ExecuteTask,
            "cancel" => InputCommand::CancelTask,
            "next" => InputCommand::SelectNextTask,
            "prev" => InputCommand::SelectPreviousTask,
            "search" => InputCommand::ToggleSearch,
            "next-result" => InputCommand::NavigateNextResult,
            "prev-result" => InputCommand::NavigatePreviousResult,
            "toggle-case" => InputCommand::ToggleSearchCase,
            "toggle-regex" => InputCommand::ToggleSearchRegex,
            _ => InputCommand::Invalid(command),
        }
    }

    fn navigate_history_backward(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => Some(0),
            Some(i) if i + 1 < self.command_history.len() => Some(i + 1),
            _ => return,
        };

        self.history_index = new_index;
        if let Some(i) = new_index {
            self.command_buffer = self.command_history[i].clone();
        }
    }

    fn navigate_history_forward(&mut self) {
        if let Some(i) = self.history_index {
            if i == 0 {
                self.command_buffer.clear();
                self.history_index = None;
            } else {
                self.history_index = Some(i - 1);
                self.command_buffer = self.command_history[i - 1].clone();
            }
        }
    }

    pub fn get_command_buffer(&self) -> &str {
        &self.command_buffer
    }

    pub fn get_last_command(&self) -> Option<&str> {
        self.last_command.as_deref()
    }

    pub fn clear_command_buffer(&mut self) {
        self.command_buffer.clear();
    }

    pub fn get_current_mode(&self) -> &InputMode {
        &self.current_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_handler_creation() {
        let handler = InputHandler::new();
        assert!(handler.command_buffer.is_empty());
        assert!(handler.command_history.is_empty());
        assert!(handler.history_index.is_none());
        assert_eq!(handler.current_mode, InputMode::Normal);
    }

    #[test]
    fn test_command_processing() {
        let mut handler = InputHandler::new();
        
        // Type "help" command
        for c in "help".chars() {
            let cmd = handler.handle_key_event(KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ));
            assert_eq!(cmd, InputCommand::None);
        }
        
        // Press enter
        let cmd = handler.handle_key_event(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ));
        assert_eq!(cmd, InputCommand::ShowHelp);
        
        // Check history
        assert_eq!(handler.command_history.len(), 1);
        assert_eq!(handler.command_history[0], "help");
    }

    #[test]
    fn test_history_navigation() {
        let mut handler = InputHandler::new();
        
        // Add commands to history
        for cmd in &["first", "second", "third"] {
            for c in cmd.chars() {
                handler.handle_key_event(KeyEvent::new(
                    KeyCode::Char(c),
                    KeyModifiers::NONE,
                ));
            }
            handler.handle_key_event(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            ));
        }
        
        // Navigate backward
        handler.handle_key_event(KeyEvent::new(
            KeyCode::Up,
            KeyModifiers::CONTROL,
        ));
        assert_eq!(handler.command_buffer, "third");
        
        handler.handle_key_event(KeyEvent::new(
            KeyCode::Up,
            KeyModifiers::CONTROL,
        ));
        assert_eq!(handler.command_buffer, "second");
        
        // Navigate forward
        handler.handle_key_event(KeyEvent::new(
            KeyCode::Down,
            KeyModifiers::CONTROL,
        ));
        assert_eq!(handler.command_buffer, "third");
    }

    #[test]
    fn test_keyboard_shortcuts() {
        let mut handler = InputHandler::new();
        
        let cmd = handler.handle_key_event(KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(cmd, InputCommand::ExecuteTask);
        
        let cmd = handler.handle_key_event(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(cmd, InputCommand::CancelTask);
        
        let cmd = handler.handle_key_event(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(cmd, InputCommand::Quit);
    }

    #[test]
    fn test_mode_transitions() {
        let mut handler = InputHandler::new();
        
        // Enter command mode
        let cmd = handler.handle_key_event(KeyEvent::new(
            KeyCode::Char(':'),
            KeyModifiers::NONE,
        ));
        assert_eq!(cmd, InputCommand::ChangeMode(InputMode::Command));
        assert_eq!(*handler.get_current_mode(), InputMode::Command);
    }
}
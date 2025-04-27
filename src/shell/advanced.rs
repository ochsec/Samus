use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::terminal::{Terminal, TerminalInstance, TerminalManager};
use crate::error::TaskError;
use crate::ui::OutputManager;

/// Represents the state of a terminal process
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessState {
    Running,
    Stopped,
    Completed(i32),
}

/// Represents a process running in the terminal
#[derive(Debug)]
pub struct TerminalProcess {
    id: u32,
    command: String,
    state: ProcessState,
    output_buffer: VecDeque<String>,
}

/// Manages terminal session state and background processes
pub struct AdvancedTerminalManager {
    inner: TerminalManager,
    processes: Arc<Mutex<HashMap<u32, TerminalProcess>>>,
    next_process_id: Arc<Mutex<u32>>,
    session_states: Arc<Mutex<HashMap<Uuid, TerminalState>>>,
}

/// Represents the preserved state of a terminal session
#[derive(Debug, Clone)]
pub struct TerminalState {
    scroll_position: usize,
    command_history: VecDeque<String>,
    environment_vars: HashMap<String, String>,
    selected_text: Option<(usize, usize)>, // Start and end positions of selection
}

impl AdvancedTerminalManager {
    pub fn new() -> Self {
        Self {
            inner: TerminalManager::new(),
            processes: Arc::new(Mutex::new(HashMap::new())),
            next_process_id: Arc::new(Mutex::new(1)),
            session_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start a process in the background
    pub fn start_background_process(
        &self,
        command: String,
    ) -> Result<u32, TaskError> {
        let mut processes = self.processes.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire processes lock".to_string())
        })?;

        let mut next_id = self.next_process_id.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire process ID lock".to_string())
        })?;

        let process_id = *next_id;
        *next_id += 1;

        let process = TerminalProcess {
            id: process_id,
            command: command.clone(),
            state: ProcessState::Running,
            output_buffer: VecDeque::with_capacity(1000),
        };

        processes.insert(process_id, process);

        Ok(process_id)
    }

    /// Stop a background process
    pub fn stop_process(&self, process_id: u32) -> Result<(), TaskError> {
        let mut processes = self.processes.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire processes lock".to_string())
        })?;

        if let Some(process) = processes.get_mut(&process_id) {
            process.state = ProcessState::Stopped;
            Ok(())
        } else {
            Err(TaskError::ExecutionFailed(format!("Process {} not found", process_id)))
        }
    }

    /// Resume a stopped process
    pub fn resume_process(&self, process_id: u32) -> Result<(), TaskError> {
        let mut processes = self.processes.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire processes lock".to_string())
        })?;

        if let Some(process) = processes.get_mut(&process_id) {
            if process.state == ProcessState::Stopped {
                process.state = ProcessState::Running;
                Ok(())
            } else {
                Err(TaskError::ExecutionFailed("Process is not stopped".to_string()))
            }
        } else {
            Err(TaskError::ExecutionFailed(format!("Process {} not found", process_id)))
        }
    }

    /// List all processes
    pub fn list_processes(&self) -> Result<Vec<(u32, String, ProcessState)>, TaskError> {
        let processes = self.processes.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire processes lock".to_string())
        })?;

        Ok(processes
            .iter()
            .map(|(id, process)| (*id, process.command.clone(), process.state.clone()))
            .collect())
    }

    /// Save terminal state for a given instance
    pub fn save_terminal_state(&self, instance: &TerminalInstance, state: TerminalState) -> Result<(), TaskError> {
        let mut states = self.session_states.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire session states lock".to_string())
        })?;

        states.insert(instance.id, state);
        Ok(())
    }

    /// Restore terminal state for a given instance
    pub fn restore_terminal_state(&self, instance: &TerminalInstance) -> Result<Option<TerminalState>, TaskError> {
        let states = self.session_states.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire session states lock".to_string())
        })?;

        Ok(states.get(&instance.id).cloned())
    }
}

impl Terminal for AdvancedTerminalManager {
    fn execute_command(&self, command: super::command::ShellCommand) -> Result<super::command::ShellCommandResult, TaskError> {
        self.inner.execute_command(command)
    }

    fn execute_streaming(
        &self,
        command: super::command::ShellCommand,
        output_mgr: Option<&OutputManager>,
        buffer_id: Option<Uuid>
    ) -> Result<mpsc::Receiver<String>, TaskError> {
        // Store output in process buffer if this is a background process
        if let Some(process) = self.processes.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire processes lock".to_string())
        })?.values_mut().find(|p| p.command == command.to_string()) {
            // Clear existing output
            process.output_buffer.clear();
        }
        
        self.inner.execute_streaming(command, output_mgr, buffer_id)
    }

    fn get_working_directory(&self) -> Result<std::path::PathBuf, TaskError> {
        self.inner.get_working_directory()
    }

    fn set_working_directory(&self, dir: std::path::PathBuf) -> Result<(), TaskError> {
        self.inner.set_working_directory(dir)
    }

    fn create_instance(&self, title: String) -> Result<TerminalInstance, TaskError> {
        let instance = self.inner.create_instance(title)?;
        
        // Initialize empty state for new instance
        let state = TerminalState {
            scroll_position: 0,
            command_history: VecDeque::new(),
            environment_vars: HashMap::new(),
            selected_text: None,
        };
        
        self.save_terminal_state(&instance, state)?;
        Ok(instance)
    }

    fn switch_to(&self, instance: &TerminalInstance) -> Result<(), TaskError> {
        self.inner.switch_to(instance)
    }

    fn clear_screen(&self) -> Result<(), TaskError> {
        self.inner.clear_screen()
    }

    fn write(&self, text: &str) -> Result<(), TaskError> {
        self.inner.write(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_background_process_management() {
        let manager = AdvancedTerminalManager::new();
        
        // Start a background process
        let process_id = manager.start_background_process("sleep 10".to_string()).unwrap();
        
        // Check process list
        let processes = manager.list_processes().unwrap();
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].0, process_id);
        assert_eq!(processes[0].1, "sleep 10");
        assert_eq!(processes[0].2, ProcessState::Running);
        
        // Stop the process
        manager.stop_process(process_id).unwrap();
        
        // Verify process state
        let processes = manager.list_processes().unwrap();
        assert_eq!(processes[0].2, ProcessState::Stopped);
        
        // Resume the process
        manager.resume_process(process_id).unwrap();
        
        // Verify process state
        let processes = manager.list_processes().unwrap();
        assert_eq!(processes[0].2, ProcessState::Running);
    }

    #[test]
    fn test_terminal_state_management() {
        let manager = AdvancedTerminalManager::new();
        
        // Create a new terminal instance
        let instance = manager.create_instance("Test Terminal".to_string()).unwrap();
        
        // Save custom state
        let state = TerminalState {
            scroll_position: 100,
            command_history: {
                let mut history = VecDeque::new();
                history.push_back("ls".to_string());
                history.push_back("cd test".to_string());
                history
            },
            environment_vars: {
                let mut vars = HashMap::new();
                vars.insert("TEST_VAR".to_string(), "test_value".to_string());
                vars
            },
            selected_text: Some((10, 20)),
        };
        
        manager.save_terminal_state(&instance, state.clone()).unwrap();
        
        // Restore and verify state
        let restored_state = manager.restore_terminal_state(&instance).unwrap().unwrap();
        assert_eq!(restored_state.scroll_position, 100);
        assert_eq!(restored_state.command_history.len(), 2);
        assert_eq!(restored_state.environment_vars.get("TEST_VAR").unwrap(), "test_value");
        assert_eq!(restored_state.selected_text, Some((10, 20)));
    }
}
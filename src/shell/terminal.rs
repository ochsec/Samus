use crate::error::TaskError;
use crate::shell::command::{ShellCommand, ShellCommandResult};
use crate::ui::OutputManager;
use crossterm::{
    cursor, execute,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
};
use std::io::stdout;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::{Mutex as AsyncMutex, mpsc};
use uuid::Uuid;

/// Represents a terminal instance with a unique identifier
#[derive(Debug, Clone)]
pub struct TerminalInstance {
    id: Uuid,
    pub title: String,
}

impl TerminalInstance {
    pub fn id(&self) -> Uuid {
        self.id
    }
}

/// Interface for interacting with a terminal.
pub trait Terminal: Send + Sync {
    /// Execute a command in the terminal.
    fn execute_command(&self, command: ShellCommand) -> Result<ShellCommandResult, TaskError>;

    /// Execute a command and stream its output.
    fn execute_streaming(
        &self,
        command: ShellCommand,
        output_mgr: Option<&OutputManager>,
        buffer_id: Option<Uuid>,
    ) -> Result<mpsc::Receiver<String>, TaskError>;

    /// Get the current working directory.
    fn get_working_directory(&self) -> Result<PathBuf, TaskError>;

    /// Set the current working directory.
    fn set_working_directory(&self, dir: PathBuf) -> Result<(), TaskError>;

    /// Create a new terminal instance
    fn create_instance(&self, title: String) -> Result<TerminalInstance, TaskError>;

    /// Switch to a specific terminal instance
    fn switch_to(&self, instance: &TerminalInstance) -> Result<(), TaskError>;

    /// Clear the current terminal screen
    fn clear_screen(&self) -> Result<(), TaskError>;

    /// Write text to the terminal at the current cursor position
    fn write(&self, text: &str) -> Result<(), TaskError>;
}

/// Terminal manager that handles multiple terminal instances
pub struct TerminalManager {
    instances: Arc<Mutex<Vec<TerminalInstance>>>,
    active_instance: Arc<AsyncMutex<Option<TerminalInstance>>>,
    working_dirs: Arc<Mutex<std::collections::HashMap<Uuid, PathBuf>>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        TerminalManager {
            instances: Arc::new(Mutex::new(Vec::new())),
            active_instance: Arc::new(AsyncMutex::new(None)),
            working_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Get all terminal instances
    pub fn get_instances(&self) -> Result<Vec<TerminalInstance>, TaskError> {
        let instances = self.instances.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for instances".to_string())
        })?;
        Ok(instances.clone())
    }
}

impl Terminal for TerminalManager {
    fn execute_command(&self, command: ShellCommand) -> Result<ShellCommandResult, TaskError> {
        let active = self.active_instance.try_lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for active instance".to_string())
        })?;

        let instance_id = active
            .as_ref()
            .ok_or_else(|| TaskError::ExecutionFailed("No active terminal instance".to_string()))?
            .id();

        let working_dirs = self.working_dirs.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for working directories".to_string())
        })?;

        let working_dir = working_dirs.get(&instance_id).ok_or_else(|| {
            TaskError::ExecutionFailed("Working directory not found for instance".to_string())
        })?;

        let command = command.working_dir(working_dir.clone());
        command.execute()
    }

    fn execute_streaming(
        &self,
        command: ShellCommand,
        output_mgr: Option<&OutputManager>,
        buffer_id: Option<Uuid>,
    ) -> Result<mpsc::Receiver<String>, TaskError> {
        let active = self.active_instance.try_lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for active instance".to_string())
        })?;

        let instance_id = active
            .as_ref()
            .ok_or_else(|| TaskError::ExecutionFailed("No active terminal instance".to_string()))?
            .id();

        let working_dirs = self.working_dirs.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for working directories".to_string())
        })?;

        let working_dir = working_dirs.get(&instance_id).ok_or_else(|| {
            TaskError::ExecutionFailed("Working directory not found for instance".to_string())
        })?;

        let command = command.working_dir(working_dir.clone());

        let runtime = tokio::runtime::Handle::current();
        let (rx, _) =
            runtime.block_on(async { command.execute_streaming(output_mgr, buffer_id).await })?;

        Ok(rx)
    }

    fn get_working_directory(&self) -> Result<PathBuf, TaskError> {
        let active = self.active_instance.try_lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for active instance".to_string())
        })?;

        let instance_id = active
            .as_ref()
            .ok_or_else(|| TaskError::ExecutionFailed("No active terminal instance".to_string()))?
            .id();

        let working_dirs = self.working_dirs.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for working directories".to_string())
        })?;

        working_dirs
            .get(&instance_id)
            .cloned()
            .ok_or_else(|| TaskError::ExecutionFailed("Working directory not found".to_string()))
    }

    fn set_working_directory(&self, dir: PathBuf) -> Result<(), TaskError> {
        let active = self.active_instance.try_lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for active instance".to_string())
        })?;

        let instance_id = active
            .as_ref()
            .ok_or_else(|| TaskError::ExecutionFailed("No active terminal instance".to_string()))?
            .id();

        let mut working_dirs = self.working_dirs.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for working directories".to_string())
        })?;

        working_dirs.insert(instance_id, dir);
        Ok(())
    }

    fn create_instance(&self, title: String) -> Result<TerminalInstance, TaskError> {
        let instance = TerminalInstance {
            id: Uuid::new_v4(),
            title,
        };

        let mut instances = self.instances.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for instances".to_string())
        })?;

        let mut working_dirs = self.working_dirs.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for working directories".to_string())
        })?;

        // Set initial working directory to current directory
        working_dirs.insert(
            instance.id,
            std::env::current_dir().map_err(|e| {
                TaskError::ExecutionFailed(format!("Failed to get current directory: {}", e))
            })?,
        );

        instances.push(instance.clone());

        // If this is the first instance, make it active
        if instances.len() == 1 {
            let mut active = self.active_instance.try_lock().map_err(|_| {
                TaskError::ExecutionFailed("Failed to acquire lock for active instance".to_string())
            })?;
            *active = Some(instance.clone());
        }

        Ok(instance)
    }

    fn switch_to(&self, instance: &TerminalInstance) -> Result<(), TaskError> {
        let instances = self.instances.lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for instances".to_string())
        })?;

        if !instances.iter().any(|i| i.id == instance.id) {
            return Err(TaskError::ExecutionFailed(
                "Terminal instance not found".to_string(),
            ));
        }

        let mut active = self.active_instance.try_lock().map_err(|_| {
            TaskError::ExecutionFailed("Failed to acquire lock for active instance".to_string())
        })?;

        *active = Some(instance.clone());
        self.clear_screen()?;

        Ok(())
    }

    fn clear_screen(&self) -> Result<(), TaskError> {
        enable_raw_mode()
            .map_err(|e| TaskError::ExecutionFailed(format!("Failed to enable raw mode: {}", e)))?;

        execute!(stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))
            .map_err(|e| TaskError::ExecutionFailed(format!("Failed to clear screen: {}", e)))?;

        disable_raw_mode().map_err(|e| {
            TaskError::ExecutionFailed(format!("Failed to disable raw mode: {}", e))
        })?;

        Ok(())
    }

    fn write(&self, text: &str) -> Result<(), TaskError> {
        use std::io::Write;

        let mut stdout = stdout();
        write!(stdout, "{}", text).map_err(|e| {
            TaskError::ExecutionFailed(format!("Failed to write to terminal: {}", e))
        })?;
        stdout.flush().map_err(|e| {
            TaskError::ExecutionFailed(format!("Failed to flush terminal output: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_terminal_instance_creation() {
        let manager = TerminalManager::new();
        let instance = manager
            .create_instance("Test Terminal".to_string())
            .unwrap();

        assert_eq!(instance.title, "Test Terminal");

        let instances = manager.get_instances().unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].title, "Test Terminal");
    }

    #[test]
    fn test_working_directory() {
        let manager = TerminalManager::new();
        let instance = manager
            .create_instance("Test Terminal".to_string())
            .unwrap();

        let test_dir = Path::new("/tmp").to_path_buf();
        manager.set_working_directory(test_dir.clone()).unwrap();

        let current_dir = manager.get_working_directory().unwrap();
        assert_eq!(current_dir, test_dir);
    }

    #[test]
    fn test_terminal_switching() {
        let manager = TerminalManager::new();
        let instance1 = manager.create_instance("Terminal 1".to_string()).unwrap();
        let instance2 = manager.create_instance("Terminal 2".to_string()).unwrap();

        manager.switch_to(&instance2).unwrap();

        let instances = manager.get_instances().unwrap();
        assert_eq!(instances.len(), 2);
    }

    #[test]
    fn test_command_execution() {
        let manager = TerminalManager::new();
        let _instance = manager
            .create_instance("Test Terminal".to_string())
            .unwrap();

        let result = manager
            .execute_command(ShellCommand::new("echo").arg("test"))
            .unwrap();

        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout.trim(), "test");
    }

    #[test]
    fn test_streaming_output() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let manager = TerminalManager::new();
        let _instance = manager
            .create_instance("Test Terminal".to_string())
            .unwrap();

        runtime.block_on(async {
            let mut rx = manager
                .execute_streaming(ShellCommand::new("echo").arg("test"), None, None)
                .unwrap();

            let output = rx.recv().await.unwrap();
            assert_eq!(output.trim(), "test");
        });
    }
}

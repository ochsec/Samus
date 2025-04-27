use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Represents a task with its metadata
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub name: String,
    // Add more task-related fields as needed
}

impl Task {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
        }
    }
}

/// Represents the output of a task
#[derive(Debug)]
pub struct TaskOutput {
    pub success: bool,
    pub message: Option<String>,
}

/// Error type for task-related operations
#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("I/O error occurred: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Task not found: {0}")]
    NotFound(String),
}

/// Trait defining task execution capabilities
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// Execute a given task
    async fn execute(&self, task: &Task) -> Result<TaskOutput, TaskError>;

    /// Cancel a task by its ID
    async fn cancel(&self, task_id: &str) -> Result<(), TaskError>;
}

/// Basic implementation of a task executor
pub struct BasicTaskExecutor {
    _tasks: Arc<Mutex<Vec<String>>>,
}

impl BasicTaskExecutor {
    pub fn new() -> Self {
        Self {
            _tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl TaskExecutor for BasicTaskExecutor {
    async fn execute(&self, task: &Task) -> Result<TaskOutput, TaskError> {
        // Placeholder implementation
        Ok(TaskOutput {
            success: true,
            message: Some(format!("Executed task: {}", task.name)),
        })
    }

    async fn cancel(&self, _task_id: &str) -> Result<(), TaskError> {
        // Placeholder implementation
        Ok(())
    }
}

// Ensure the trait is implemented for BasicTaskExecutor
impl Default for BasicTaskExecutor {
    fn default() -> Self {
        Self::new()
    }
}

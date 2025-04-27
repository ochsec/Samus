use async_trait::async_trait;
use crate::error::TaskError;
use crate::task::Task;

/// Output type for task execution
#[derive(Debug)]
pub struct TaskOutput {
    pub success: bool,
    pub message: Option<String>,
}

/// Trait defining task execution capabilities for MCP
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// Execute a given task
    async fn execute(&self, task: &Task) -> Result<TaskOutput, TaskError>;

    /// Cancel a task by its ID
    async fn cancel(&self, task_id: &str) -> Result<(), TaskError>;
}

/// Basic implementation of a task executor
pub struct BasicTaskExecutor {}

impl BasicTaskExecutor {
    pub fn new() -> Self {
        Self {}
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

impl Default for BasicTaskExecutor {
    fn default() -> Self {
        Self::new()
    }
}
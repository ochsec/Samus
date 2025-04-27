use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::TaskError;
use crate::fs::operations::FileSystem;

pub mod tree_sitter_task;

/// Unique identifier for tasks
pub type TaskId = String;

/// Result of a task execution
#[derive(Debug, Clone)]
pub enum TaskResult {
    Json(Value),
    Text(String),
    Binary(Vec<u8>),
}

/// Context provided to task handlers
pub struct TaskContext {
    pub fs: Arc<dyn FileSystem + Send + Sync>,
    pub task_manager: Arc<dyn TaskManagerTrait>,
    // Add other context elements like config, etc.
}

/// Task definition with parameters
#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub params: Value,
}

impl Task {
    pub fn new(name: &str, params: Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            params,
        }
    }
}

/// Handler for task execution
#[async_trait]
pub trait TaskHandler: Send + Sync {
    async fn handle_task(&self, task: Task, ctx: &TaskContext) -> Result<TaskResult, TaskError>;
}

/// Registry of task handlers
pub struct TaskRegistry {
    handlers: HashMap<String, Arc<dyn TaskHandler>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, handler: Arc<dyn TaskHandler>) {
        self.handlers.insert(name.to_string(), handler);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn TaskHandler>> {
        self.handlers.get(name).cloned()
    }
}

/// Manager for executing tasks
pub struct TaskManager {
    registry: Arc<TaskRegistry>,
    context: TaskContext,
}

impl TaskManager {
    pub fn new(fs: Arc<dyn FileSystem + Send + Sync>, registry: Arc<TaskRegistry>) -> Self {
        // Create a minimal context first
        let context = TaskContext {
            fs: fs.clone(),
            task_manager: Arc::new(TaskManagerPlaceholder {}),
        };
        
        Self {
            registry,
            context,
        }
    }
}

#[async_trait]
impl TaskManagerTrait for TaskManager {
    async fn execute_task(&self, task: Task) -> Result<TaskResult, TaskError> {
        let handler = self.registry.get(&task.name)
            .ok_or_else(|| TaskError::HandlerNotFound(task.name.clone()))?;
        
        handler.handle_task(task, &self.context).await
    }
}

// Placeholder for solving circular reference in TaskContext
struct TaskManagerPlaceholder;

// Trait to allow dynamic dispatch for TaskManager
#[async_trait]
pub trait TaskManagerTrait: Send + Sync {
    async fn execute_task(&self, task: Task) -> Result<TaskResult, TaskError>;
}

#[async_trait]
impl TaskManagerTrait for TaskManagerPlaceholder {
    async fn execute_task(&self, _task: Task) -> Result<TaskResult, TaskError> {
        Err(TaskError::NotInitialized)
    }
}
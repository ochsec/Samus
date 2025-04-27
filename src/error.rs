use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Task execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Resource unavailable: {0}")]
    ResourceUnavailable(String),
    
    #[error("Task was cancelled")]
    Cancelled,
    
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("File system error: {0}")]
    FileSystem(String),
    
    #[error("Tool error: {0}")]
    Tool(String),
    
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    
    #[error("Task handler not found: {0}")]
    HandlerNotFound(String),
    
    #[error("Task manager not initialized")]
    NotInitialized,
}

// Note: The #[derive(Error)] above already implements Display and Error trait
// as well as the From<std::io::Error> conversion.

impl From<serde_json::Error> for TaskError {
    fn from(err: serde_json::Error) -> Self {
        TaskError::SerializationError(err.to_string())
    }
}

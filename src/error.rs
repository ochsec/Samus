use std::fmt;

#[derive(Debug)]
pub enum TaskError {
    ExecutionFailed(String),
    ResourceUnavailable(String),
    Cancelled,
    InvalidConfiguration(String),
    IoError(std::io::Error),
    SerializationError(String),
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::ExecutionFailed(msg) => write!(f, "Task execution failed: {}", msg),
            TaskError::ResourceUnavailable(msg) => write!(f, "Resource unavailable: {}", msg),
            TaskError::Cancelled => write!(f, "Task was cancelled"),
            TaskError::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
            TaskError::IoError(err) => write!(f, "IO error: {}", err),
            TaskError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for TaskError {}

impl From<std::io::Error> for TaskError {
    fn from(err: std::io::Error) -> Self {
        TaskError::IoError(err)
    }
}

impl From<serde_json::Error> for TaskError {
    fn from(err: serde_json::Error) -> Self {
        TaskError::SerializationError(err.to_string())
    }
}

/// Types to support the UI task view

/// Represents the output of a task
#[derive(Debug, Clone)]
pub struct TaskOutput {
    pub success: bool,
    pub message: Option<String>,
}
use crate::error::TaskError;
use crate::task::{Task, TaskContext, TaskHandler, TaskResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ShellTaskRequest {
    #[serde(rename = "execute")]
    Execute {
        command: String,
        args: Option<Vec<String>>,
        #[serde(default)]
        capture_stderr: bool,
    },
    #[serde(rename = "list_directory")]
    ListDirectory {
        path: String,
        #[serde(default)]
        recursive: bool,
    },
}

#[derive(Debug, Serialize)]
pub struct ShellTaskResponse {
    pub output: String,
    pub exit_code: Option<i32>,
    pub success: bool,
}

pub struct ShellTaskHandler;

impl ShellTaskHandler {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl TaskHandler for ShellTaskHandler {
    async fn handle_task(&self, task: Task, _ctx: &TaskContext) -> Result<TaskResult, TaskError> {
        // Deserialize the task request
        let request: ShellTaskRequest = serde_json::from_value(task.params)
            .map_err(|e| TaskError::InvalidParameter(format!("Invalid parameters: {}", e)))?;
        
        match request {
            ShellTaskRequest::Execute {
                command,
                args,
                capture_stderr,
            } => {
                // Build command
                let args = args.unwrap_or_default();
                
                // Run command
                let output = if cfg!(target_os = "windows") {
                    let mut cmd = Command::new("cmd");
                    cmd.arg("/C").arg(&command);
                    for arg in args {
                        cmd.arg(arg);
                    }
                    if capture_stderr {
                        cmd.stderr(std::process::Stdio::piped());
                    }
                    cmd.output()
                } else {
                    let mut cmd = Command::new(&command);
                    for arg in args {
                        cmd.arg(arg);
                    }
                    if capture_stderr {
                        cmd.stderr(std::process::Stdio::piped());
                    }
                    cmd.output()
                };
                
                match output {
                    Ok(output) => {
                        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        
                        if capture_stderr && !output.stderr.is_empty() {
                            stdout.push_str("\nSTDERR:\n");
                            stdout.push_str(&String::from_utf8_lossy(&output.stderr));
                        }
                        
                        let response = ShellTaskResponse {
                            output: stdout,
                            exit_code: output.status.code(),
                            success: output.status.success(),
                        };
                        
                        Ok(TaskResult::Json(serde_json::to_value(response).unwrap()))
                    },
                    Err(e) => {
                        Err(TaskError::ExecutionFailed(format!("Failed to execute command: {}", e)))
                    }
                }
            },
            
            ShellTaskRequest::ListDirectory { path, recursive } => {
                // Use find or ls command depending on platform and recursive flag
                let (command, args) = if cfg!(target_os = "windows") {
                    if recursive {
                        ("cmd", vec!["/C", "dir", "/S", "/B", &path])
                    } else {
                        ("cmd", vec!["/C", "dir", "/B", &path])
                    }
                } else {
                    if recursive {
                        ("find", vec![&path, "-type", "f", "-o", "-type", "d"])
                    } else {
                        ("ls", vec!["-la", &path])
                    }
                };
                
                // Run command
                let mut cmd = Command::new(command);
                for arg in args {
                    cmd.arg(arg);
                }
                let cmd_result = cmd.output();
                
                match cmd_result {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        
                        let response = ShellTaskResponse {
                            output: stdout,
                            exit_code: output.status.code(),
                            success: output.status.success(),
                        };
                        
                        Ok(TaskResult::Json(serde_json::to_value(response).unwrap()))
                    },
                    Err(e) => {
                        Err(TaskError::ExecutionFailed(format!("Failed to list directory: {}", e)))
                    }
                }
            }
        }
    }
}
use crate::error::TaskError;
use crate::ui::OutputManager;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as AsyncCommand;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[cfg(target_family = "unix")]
use std::env::var_os;

/// Result of executing a shell command.
#[derive(Debug, Clone)]
pub struct ShellCommandResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Default command timeout in seconds
const DEFAULT_TIMEOUT: u64 = 300;

/// Platform-specific shell detection
#[cfg(target_family = "unix")]
fn detect_shell() -> (String, Vec<String>) {
    let shell = var_os("SHELL")
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "/bin/sh".to_string());

    (shell, vec!["-c".to_string()])
}

#[cfg(target_family = "windows")]
fn detect_shell() -> (String, Vec<String>) {
    ("cmd.exe".to_string(), vec!["/C".to_string()])
}

/// A shell command that can be executed.
pub struct ShellCommand {
    program: String,
    args: Vec<String>,
    working_dir: Option<PathBuf>,
    env_vars: Vec<(String, String)>,
    timeout_secs: u64,
    use_shell: bool,
}

impl ShellCommand {
    pub fn new(program: &str) -> Self {
        ShellCommand {
            program: program.to_string(),
            args: Vec::new(),
            working_dir: None,
            env_vars: Vec::new(),
            timeout_secs: DEFAULT_TIMEOUT,
            use_shell: false,
        }
    }

    pub fn arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    pub fn args(mut self, args: &[&str]) -> Self {
        self.args.extend(args.iter().map(|s| s.to_string()));
        self
    }

    pub fn working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env_vars.push((key.to_string(), value.to_string()));
        self
    }

    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    pub fn use_shell(mut self, enabled: bool) -> Self {
        self.use_shell = enabled;
        self
    }

    /// Execute the command and return the result.
    pub fn execute(&self) -> Result<ShellCommandResult, TaskError> {
        let (mut cmd, args) = if self.use_shell {
            let (shell, shell_args) = detect_shell();
            let mut full_command = self.program.clone();
            for arg in &self.args {
                full_command.push(' ');
                full_command.push_str(arg);
            }

            let cmd = Command::new(shell);
            let mut args = shell_args;
            args.push(full_command);
            (cmd, args)
        } else {
            (Command::new(&self.program), self.args.clone())
        };

        cmd.args(args);

        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in &self.env_vars {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .map_err(|e| TaskError::ExecutionFailed(format!("Failed to execute command: {}", e)))?;

        let result = ShellCommandResult {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            timed_out: false,
        };

        Ok(result)
    }

    /// Execute the command asynchronously and stream the output.
    pub async fn execute_streaming(
        &self,
        output_mgr: Option<&OutputManager>,
        buffer_id: Option<uuid::Uuid>,
    ) -> Result<(mpsc::Receiver<String>, ShellCommandResult), TaskError> {
        let (shell_cmd, args) = if self.use_shell {
            let (shell, shell_args) = detect_shell();
            let mut full_command = self.program.clone();
            for arg in &self.args {
                full_command.push(' ');
                full_command.push_str(arg);
            }

            let mut args = shell_args;
            args.push(full_command);
            (shell, args)
        } else {
            (self.program.clone(), self.args.clone())
        };

        let mut cmd = AsyncCommand::new(shell_cmd);
        cmd.args(args);

        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in &self.env_vars {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| TaskError::ExecutionFailed(format!("Failed to execute command: {}", e)))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TaskError::ExecutionFailed("Failed to capture stdout".to_string()))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| TaskError::ExecutionFailed("Failed to capture stderr".to_string()))?;

        let (tx, rx) = mpsc::channel(100);
        let tx_clone = tx.clone();

        // Set up output handling
        let output_sender = if let (Some(mgr), Some(_id)) = (output_mgr, buffer_id) {
            Some(mgr.get_sender())
        } else {
            None
        };

        // Spawn a task to read stdout
        if let Some(sender) = output_sender.clone() {
            let buffer_id = buffer_id.unwrap();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx.send(line.clone()).await;
                    if let Some(ref sender) = sender {
                        let _ = sender
                            .send(format!("[stdout:{}] {}", buffer_id, line))
                            .await;
                    }
                }
            });
        } else {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx.send(line).await;
                }
            });
        }

        // Spawn a task to read stderr
        if let Some(sender) = output_sender {
            let buffer_id = buffer_id.unwrap();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx_clone.send(line.clone()).await;
                    if let Some(ref sender) = sender {
                        let _ = sender
                            .send(format!("[stderr:{}] {}", buffer_id, line))
                            .await;
                    }
                }
            });
        } else {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = tx_clone.send(line).await;
                }
            });
        }

        // Wait for the command to complete with timeout
        let status = match timeout(Duration::from_secs(self.timeout_secs), child.wait()).await {
            Ok(result) => result.map_err(|e| {
                TaskError::ExecutionFailed(format!("Failed to wait for command: {}", e))
            })?,
            Err(_) => {
                let _ = child.kill().await;
                return Ok((
                    rx,
                    ShellCommandResult {
                        exit_code: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        timed_out: true,
                    },
                ));
            }
        };

        let result = ShellCommandResult {
            exit_code: status.code(),
            stdout: String::new(), // Content is streamed via the channel
            stderr: String::new(), // Content is streamed via the channel
            timed_out: false,
        };

        Ok((rx, result))
    }

    /// Spawn an asynchronous command and return the child process handle
    pub fn spawn(&self) -> Result<tokio::process::Child, TaskError> {
        let (shell_cmd, args) = if self.use_shell {
            let (shell, mut shell_args) = detect_shell();
            let mut full_command = self.program.clone();
            for arg in &self.args {
                full_command.push(' ');
                full_command.push_str(arg);
            }

            shell_args.push(full_command);
            (shell, shell_args)
        } else {
            (self.program.clone(), self.args.clone())
        };

        let mut cmd = AsyncCommand::new(shell_cmd);
        cmd.args(args);

        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in &self.env_vars {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        cmd.spawn()
            .map_err(|e| TaskError::ExecutionFailed(format!("Failed to spawn command: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tokio::runtime::Runtime;

    #[test]
    fn test_basic_command() {
        let cmd = ShellCommand::new("echo").arg("hello").execute().unwrap();

        assert_eq!(cmd.exit_code, Some(0));
        assert_eq!(cmd.stdout.trim(), "hello");
        assert!(cmd.stderr.is_empty());
    }

    #[test]
    fn test_command_with_env() {
        let cmd = ShellCommand::new("printenv")
            .env("TEST_VAR", "test_value")
            .arg("TEST_VAR")
            .execute()
            .unwrap();

        assert_eq!(cmd.exit_code, Some(0));
        assert_eq!(cmd.stdout.trim(), "test_value");
    }

    #[test]
    fn test_command_timeout() {
        let rt = Runtime::new().unwrap();
        let start = Instant::now();

        let (_, result) = rt.block_on(async {
            ShellCommand::new("sleep")
                .arg("5")
                .timeout(1)
                .execute_streaming(None, None)
                .await
                .unwrap()
        });

        let elapsed = start.elapsed();
        assert!(result.timed_out);
        assert!(elapsed.as_secs() < 5);
    }

    #[test]
    fn test_shell_command() {
        let cmd = ShellCommand::new("echo $PATH")
            .use_shell(true)
            .execute()
            .unwrap();

        assert_eq!(cmd.exit_code, Some(0));
        assert!(!cmd.stdout.trim().is_empty());
    }

    #[test]
    fn test_streaming_output() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let (mut rx, result) = ShellCommand::new("echo")
                .args(&["line1", "line2"])
                .execute_streaming(None, None)
                .await
                .unwrap();

            assert_eq!(result.exit_code, Some(0));

            let mut output = Vec::new();
            while let Some(line) = rx.recv().await {
                output.push(line);
            }

            assert_eq!(output.len(), 1);
            assert_eq!(output[0], "line1 line2");
        });
    }

    #[test]
    fn test_streaming_with_output_manager() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let mut output_mgr = OutputManager::new();
            let buffer_id = output_mgr.create_buffer();

            let (mut rx, result) = ShellCommand::new("echo")
                .arg("test output")
                .execute_streaming(Some(&output_mgr), Some(buffer_id))
                .await
                .unwrap();

            // Process the output
            output_mgr.process_output().await;

            // Verify command result
            assert_eq!(result.exit_code, Some(0));

            // Verify output in rx channel
            let mut output = Vec::new();
            while let Some(line) = rx.recv().await {
                output.push(line);
            }
            assert_eq!(output.len(), 1);
            assert_eq!(output[0], "test output");

            // Verify output in buffer
            let lines = output_mgr.get_lines();
            assert!(!lines.is_empty());
            assert!(lines.iter().any(|line| line.contains("test output")));
        });
    }

    #[test]
    fn test_stderr_streaming() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let mut output_mgr = OutputManager::new();
            let buffer_id = output_mgr.create_buffer();

            let (mut rx, result) = ShellCommand::new("sh")
                .use_shell(true)
                .arg("-c")
                .arg("echo error >&2")
                .execute_streaming(Some(&output_mgr), Some(buffer_id))
                .await
                .unwrap();

            // Process the output
            output_mgr.process_output().await;

            // Verify command result
            assert_eq!(result.exit_code, Some(0));

            // Verify stderr in buffer
            let lines = output_mgr.get_lines();
            assert!(!lines.is_empty());
            assert!(lines.iter().any(|line| line.contains("error")));
        });
    }
}

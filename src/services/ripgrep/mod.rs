use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Error types specific to Ripgrep operations
#[derive(Debug, thiserror::Error)]
pub enum RipgrepError {
    #[error("Ripgrep binary not found")]
    BinaryNotFound,
    #[error("Failed to execute ripgrep: {0}")]
    ExecutionError(String),
    #[error("Invalid regex pattern: {0}")]
    InvalidPattern(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Configuration for search operations
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub pattern: String,
    pub file_pattern: Option<String>,
    pub context_lines: usize,
    pub max_results: usize,
    pub max_line_length: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            file_pattern: None,
            context_lines: 2,
            max_results: 300,
            max_line_length: 500,
        }
    }
}

/// A single search result with context
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub line_content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

/// The main Ripgrep service for high-performance file searching
pub struct RipgrepService {
    binary_path: PathBuf,
    result_count: Arc<AtomicUsize>,
}

impl RipgrepService {
    /// Create a new RipgrepService instance
    pub fn new() -> Result<Self, RipgrepError> {
        let binary_path = Self::detect_binary()?;
        Ok(Self {
            binary_path,
            result_count: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Detect the ripgrep binary location using multiple fallback paths
    fn detect_binary() -> Result<PathBuf, RipgrepError> {
        // VSCode bundled ripgrep paths
        let vscode_paths = if cfg!(target_os = "windows") {
            vec![
                "C:\\Program Files\\Microsoft VS Code\\resources\\app\\node_modules\\vscode-ripgrep\\bin\\rg.exe",
                "C:\\Program Files (x86)\\Microsoft VS Code\\resources\\app\\node_modules\\vscode-ripgrep\\bin\\rg.exe",
            ]
        } else {
            vec![
                "/usr/share/code/resources/app/node_modules/vscode-ripgrep/bin/rg",
                "/Applications/Visual Studio Code.app/Contents/Resources/app/node_modules/vscode-ripgrep/bin/rg",
            ]
        };

        // Check VSCode paths first
        for path in vscode_paths {
            if Path::new(path).exists() {
                return Ok(PathBuf::from(path));
            }
        }

        // Try system PATH
        if let Ok(output) = Command::new("which").arg("rg").output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    let path = path.trim();
                    return Ok(PathBuf::from(path));
                }
            }
        }

        Err(RipgrepError::BinaryNotFound)
    }

    /// Execute a search with the given configuration
    pub fn search<P: AsRef<Path>>(
        &self,
        dir: P,
        config: SearchConfig,
        callback: impl FnMut(SearchResult) -> bool,
    ) -> Result<usize, RipgrepError> {
        // Validate regex pattern
        if let Err(e) = regex::Regex::new(&config.pattern) {
            return Err(RipgrepError::InvalidPattern(e.to_string()));
        }

        let mut cmd = Command::new(&self.binary_path);
        cmd.current_dir(dir)
            .arg("--line-number")
            .arg("--context")
            .arg(config.context_lines.to_string())
            .arg("--color")
            .arg("never")
            .arg("--text"); // Force text mode

        // Add file pattern if specified
        if let Some(pattern) = config.file_pattern {
            cmd.arg("--glob").arg(pattern);
        }

        // Add the search pattern
        cmd.arg(&config.pattern);

        // Configure stdio
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut process = cmd.spawn()?;
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| RipgrepError::ExecutionError("Failed to capture stdout".to_string()))?;

        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let mut current_file = None;
        let mut current_results = Vec::new();
        let mut context_buffer = Vec::new();
        let mut in_context = false;
        let mut callback = callback;

        while reader.read_line(&mut line)? > 0 {
            // Check result limit
            if self.result_count.load(Ordering::Relaxed) >= config.max_results {
                break;
            }

            // Process line
            if line.starts_with("--") {
                // Context separator
                in_context = true;
            } else if let Some((file_path, line_num, content)) = self.parse_result_line(&line) {
                if current_file.as_ref() != Some(&file_path) {
                    // New file
                    self.flush_results(&mut current_results, &mut callback);
                    current_file = Some(file_path.clone());
                    current_results.clear();
                }

                let mut result = SearchResult {
                    file_path,
                    line_number: line_num,
                    line_content: self.truncate_line(&content, config.max_line_length),
                    context_before: Vec::new(),
                    context_after: Vec::new(),
                };

                if in_context {
                    result.context_before = context_buffer.clone();
                }

                context_buffer.clear();
                current_results.push(result);

                self.result_count.fetch_add(1, Ordering::Relaxed);
            } else {
                // Context line
                context_buffer.push(self.truncate_line(&line, config.max_line_length));
                if context_buffer.len() > config.context_lines {
                    context_buffer.remove(0);
                }
            }

            line.clear();
        }

        // Flush any remaining results
        self.flush_results(&mut current_results, &mut callback);

        Ok(self.result_count.load(Ordering::Relaxed))
    }

    /// Parse a result line into (file_path, line_number, content)
    fn parse_result_line(&self, line: &str) -> Option<(PathBuf, usize, String)> {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() != 3 {
            return None;
        }

        let file_path = PathBuf::from(parts[0]);
        let line_number = parts[1].parse().ok()?;
        let content = parts[2].trim_end().to_string();

        Some((file_path, line_number, content))
    }

    /// Truncate a line to the maximum length
    fn truncate_line(&self, line: &str, max_length: usize) -> String {
        if line.len() <= max_length {
            line.to_string()
        } else {
            format!("{}...", &line[..max_length - 3])
        }
    }

    /// Flush accumulated results through the callback
    fn flush_results(
        &self,
        results: &mut Vec<SearchResult>,
        callback: &mut impl FnMut(SearchResult) -> bool,
    ) {
        for result in results.drain(..) {
            if !callback(result) {
                break;
            }
        }
    }

    /// Reset the result counter
    pub fn reset_count(&self) {
        self.result_count.store(0, Ordering::Relaxed);
    }

    /// Get the current result count
    pub fn get_count(&self) -> usize {
        self.result_count.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_ripgrep_search() -> Result<(), Box<dyn std::error::Error>> {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");

        // Create a test file
        let mut file = File::create(&file_path)?;
        writeln!(file, "Line 1: test content")?;
        writeln!(file, "Line 2: more content")?;
        writeln!(file, "Line 3: test pattern")?;
        writeln!(file, "Line 4: final line")?;

        // Initialize RipgrepService
        let service = RipgrepService::new()?;

        // Configure search
        let config = SearchConfig {
            pattern: String::from("test"),
            file_pattern: Some(String::from("*.txt")),
            context_lines: 1,
            max_results: 10,
            max_line_length: 100,
        };

        // Collect results
        let mut results = Vec::new();
        service.search(temp_dir.path(), config, |result| {
            results.push(result);
            true
        })?;

        // Verify results
        assert!(!results.is_empty());
        assert_eq!(service.get_count(), results.len());

        Ok(())
    }
}

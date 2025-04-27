use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::error::TaskError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub app_name: String,
    pub log_level: LogLevel,
    pub mcp_servers: Vec<McpServerConfig>,
    pub terminal: TerminalConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub api_key: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalConfig {
    pub default_shell: Option<String>,
    pub history_limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            app_name: "samus".to_string(),
            log_level: LogLevel::Info,
            mcp_servers: Vec::new(),
            terminal: TerminalConfig {
                default_shell: None,
                history_limit: 1000,
            },
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn get_usize(&self, key: &str) -> Option<usize> {
        match key {
            "terminal.history_limit" => Some(self.terminal.history_limit),
            "tree_sitter.max_file_size" => Some(5 * 1024 * 1024), // 5MB default
            "tree_sitter.max_parsers_per_lang" => Some(4),        // 4 parsers per language default
            _ => None,
        }
    }
    
    pub fn get_string(&self, key: &str) -> Option<String> {
        match key {
            "app_name" => Some(self.app_name.clone()),
            _ => None,
        }
    }

    pub fn load(path: &Path) -> Result<Self, TaskError> {
        if !path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(path).map_err(|e| TaskError::IoError(e))?;

        serde_json::from_str(&content).map_err(|e| TaskError::InvalidConfiguration(e.to_string()))
    }

    pub fn save(&self, path: &Path) -> Result<(), TaskError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| TaskError::IoError(e))?;
            }
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| TaskError::InvalidConfiguration(e.to_string()))?;

        fs::write(path, content).map_err(|e| TaskError::IoError(e))
    }
}

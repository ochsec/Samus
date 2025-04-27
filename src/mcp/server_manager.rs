use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::TaskError;
use crate::fs::FileSystemOperations;
use crate::mcp::protocol::ServerState;
use crate::mcp::task_executor::TaskExecutor;
use crate::task::Task;

/// Configuration for an MCP server
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    pub working_dir: Option<String>,
    pub env: HashMap<String, String>,
    pub restart_policy: RestartPolicy,
}

/// Server restart policies
#[derive(Debug, Clone, PartialEq)]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

/// Represents a running MCP server instance
#[derive(Debug)]
pub struct ServerInstance {
    pub config: ServerConfig,
    pub state: ServerState,
    pub process: Option<tokio::process::Child>,
    pub last_error: Option<String>,
}

/// Manages MCP server lifecycles and operations
pub struct ServerManager {
    servers: Arc<RwLock<HashMap<String, ServerInstance>>>,
    fs: Arc<dyn FileSystemOperations>,
    executor: Arc<dyn TaskExecutor>,
}

// Rest of the existing implementation remains the same
impl ServerManager {
    pub fn new(fs: Arc<dyn FileSystemOperations>, executor: Arc<dyn TaskExecutor>) -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            fs,
            executor,
        }
    }

    // ... (existing methods)
}

// Existing tests and other code remain the same

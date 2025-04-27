pub mod editor;
pub mod mock_vscode;

// Alias for providing vscode-like functionality
pub mod vscode {
    pub use super::mock_vscode::*;
}

use anyhow::Context;
use std::path::PathBuf;

/// Custom error types for VSCode integrations
#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    #[error("Workspace not found")]
    WorkspaceNotFound,

    #[error("Failed to initialize editor integration: {0}")]
    EditorInitError(String),

    #[error("Integration configuration error: {0}")]
    ConfigError(String),
}

/// Represents the VSCode workspace configuration
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Root path of the workspace
    pub root_path: PathBuf,
    /// Name of the workspace
    pub name: String,
}

/// Module containing VSCode integration components and providers
#[derive(Debug, Default)]
pub struct Integrations {
    /// Current workspace configuration
    workspace: Option<WorkspaceConfig>,
    /// Initialization status
    initialized: bool,
}

impl Integrations {
    /// Create a new Integrations instance
    pub fn new() -> Self {
        Self {
            workspace: None,
            initialized: false,
        }
    }

    /// Initialize all VSCode integrations
    pub async fn init() -> anyhow::Result<()> {
        let instance = Self::new();
        instance
            .detect_workspace()
            .context("Failed to detect workspace")?;

        // Initialize editor integration
        editor::EditorIntegration::init()
            .await
            .context("Failed to initialize editor integration")?;

        Ok(())
    }

    /// Detect and configure the current VSCode workspace
    fn detect_workspace(&self) -> Result<WorkspaceConfig, IntegrationError> {
        // TODO: Implement actual workspace detection
        // For now, use current directory as workspace root
        let workspace_root =
            std::env::current_dir().map_err(|e| IntegrationError::WorkspaceNotFound)?;

        let name = workspace_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(WorkspaceConfig {
            root_path: workspace_root,
            name,
        })
    }

    /// Check if integrations are initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get current workspace configuration if available
    pub fn workspace(&self) -> Option<&WorkspaceConfig> {
        self.workspace.as_ref()
    }

    /// Extension point for registering new integration features
    pub async fn register_feature<F>(&mut self, _feature: F) -> anyhow::Result<()>
    where
        F: IntegrationFeature,
    {
        // TODO: Implement feature registration
        Ok(())
    }
}

/// Trait for implementing new integration features
pub trait IntegrationFeature: Send + Sync {
    /// Initialize the feature
    fn init(&self) -> anyhow::Result<()>;

    /// Clean up the feature
    fn cleanup(&self) -> anyhow::Result<()>;
}

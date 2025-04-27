mod decoration_controller;
mod diff_view_provider;

pub use decoration_controller::DecorationController;

use crate::integrations::{IntegrationError, IntegrationFeature};

/// Module for VSCode editor integration components
#[derive(Debug)]
pub struct EditorIntegration {
    decoration_controller: DecorationController,
    initialized: bool,
}

impl EditorIntegration {
    /// Create a new EditorIntegration instance
    pub fn new() -> Self {
        Self {
            decoration_controller: DecorationController::new(),
            initialized: false,
        }
    }

    /// Initialize editor integrations
    pub async fn init() -> anyhow::Result<()> {
        let mut integration = Self::new();

        // Initialize decoration controller
        integration
            .decoration_controller
            .register_decoration_types()
            .await
            .map_err(|e| IntegrationError::EditorInitError(e.to_string()))?;

        integration.initialized = true;
        Ok(())
    }

    /// Get a reference to the decoration controller
    pub fn decoration_controller(&self) -> &DecorationController {
        &self.decoration_controller
    }

    /// Get a mutable reference to the decoration controller
    pub fn decoration_controller_mut(&mut self) -> &mut DecorationController {
        &mut self.decoration_controller
    }

    /// Check if the editor integration is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl IntegrationFeature for EditorIntegration {
    fn init(&self) -> anyhow::Result<()> {
        if !self.initialized {
            return Err(IntegrationError::EditorInitError(
                "Editor integration not initialized".to_string(),
            )
            .into());
        }
        Ok(())
    }

    fn cleanup(&self) -> anyhow::Result<()> {
        // Clean up any editor resources
        Ok(())
    }
}

impl Default for EditorIntegration {
    fn default() -> Self {
        Self::new()
    }
}

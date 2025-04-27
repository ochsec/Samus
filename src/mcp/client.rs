use reqwest::{Client as HttpClient, header};
use serde_json::json;

use super::protocol::Version;
use crate::config::McpServerConfig;
use crate::error::TaskError;

// Define ToolDefinition with the necessary fields
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub version: Version,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct OpenRouterClient {
    http_client: HttpClient,
    config: McpServerConfig,
    model: String,
}

impl OpenRouterClient {
    pub fn new(config: McpServerConfig, model: String) -> Result<Self, TaskError> {
        // Ensure API key is available
        let api_key = config.api_key.clone().ok_or_else(|| {
            TaskError::InvalidConfiguration("OpenRouter API key is missing".to_string())
        })?;

        // Create HTTP client with authorization header
        let mut headers = header::HeaderMap::new();
        let auth_value = format!("Bearer {}", api_key);
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&auth_value)
                .map_err(|e| TaskError::InvalidConfiguration(e.to_string()))?,
        );

        let http_client = HttpClient::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| TaskError::InvalidConfiguration(e.to_string()))?;

        Ok(Self {
            http_client,
            config,
            model,
        })
    }

    pub async fn chat(&self, prompt: String) -> Result<String, TaskError> {
        // Check if prompt is empty
        if prompt.trim().is_empty() {
            return Err(TaskError::ExecutionFailed(
                "Input must have at least 1 token".to_string(),
            ));
        }

        // Prepare request payload for OpenRouter
        let payload = json!({
            "model": self.model.clone(),
            "messages": [
                {
                    "role": "user",
                    "content": prompt,
                }
            ]
        });

        // Send request to OpenRouter
        let response = self
            .http_client
            .post(&self.config.url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| TaskError::ExecutionFailed(format!("Failed to send request: {}", e)))?;

        // Handle response
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.map_err(|e| {
                TaskError::ExecutionFailed(format!("Failed to read error response: {}", e))
            })?;
            return Err(TaskError::ExecutionFailed(format!(
                "OpenRouter request failed with status {}: {}",
                status, error_text
            )));
        }

        // Get the response body as text first
        let response_body = response.text().await.map_err(|e| {
            TaskError::ExecutionFailed(format!("Failed to read response body: {}", e))
        })?;

        // Parse the JSON
        let response_json: serde_json::Value =
            serde_json::from_str(&response_body).map_err(|e| {
                TaskError::ExecutionFailed(format!("Failed to parse response JSON: {}", e))
            })?;

        // Extract assistant message - more flexible approach to handle different provider formats
        let content = if let Some(choices) = response_json.get("choices") {
            if let Some(first_choice) = choices.get(0) {
                if let Some(message) = first_choice.get("message") {
                    if let Some(content) = message.get("content") {
                        if let Some(text) = content.as_str() {
                            text.to_string()
                        } else {
                            return Err(TaskError::ExecutionFailed(
                                "Content is not a string".to_string(),
                            ));
                        }
                    } else {
                        // Try alternative formats
                        if let Some(text) = first_choice.get("text").and_then(|t| t.as_str()) {
                            text.to_string()
                        } else if let Some(content) =
                            first_choice.get("content").and_then(|c| c.as_str())
                        {
                            content.to_string()
                        } else {
                            return Err(TaskError::ExecutionFailed(format!(
                                "Could not find content in response: {:?}",
                                first_choice
                            )));
                        }
                    }
                } else {
                    // Try to get text directly from choice
                    if let Some(text) = first_choice.get("text").and_then(|t| t.as_str()) {
                        text.to_string()
                    } else if let Some(content) =
                        first_choice.get("content").and_then(|c| c.as_str())
                    {
                        content.to_string()
                    } else {
                        return Err(TaskError::ExecutionFailed(format!(
                            "Response missing message field: {:?}",
                            first_choice
                        )));
                    }
                }
            } else {
                return Err(TaskError::ExecutionFailed(
                    "Choices array is empty".to_string(),
                ));
            }
        } else {
            return Err(TaskError::ExecutionFailed(format!(
                "Response missing choices field: {:?}",
                response_json
            )));
        };

        Ok(content)
    }

    pub fn get_model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }
}

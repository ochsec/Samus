use crate::error::TaskError;
use crate::services::tree_sitter::{SupportedLanguage, TreeSitterService};
use crate::task::{Task, TaskContext, TaskHandler, TaskId, TaskResult};
use crate::tools::{
    parse_file, parse_code_string, search_definitions, search_components, run_custom_query,
    CodeSearchError, TreeParserError,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

// Task request types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum TreeSitterTaskRequest {
    #[serde(rename = "parse_file")]
    ParseFile {
        file_path: String,
    },
    #[serde(rename = "search_definitions")]
    SearchDefinitions {
        file_path: String,
    },
    #[serde(rename = "search_components")]
    SearchComponents {
        file_path: String,
    },
    #[serde(rename = "custom_query")]
    CustomQuery {
        file_path: String,
        query: String,
    },
    #[serde(rename = "parse_string")]
    ParseString {
        content: String,
        language: String,
    },
}

// Task handler for tree-sitter operations
pub struct TreeSitterTaskHandler {
    service: Arc<TreeSitterService>,
}

impl TreeSitterTaskHandler {
    pub fn new(service: Arc<TreeSitterService>) -> Self {
        Self { service }
    }
    
    // Helper to convert language string to enum
    fn parse_language(lang: &str) -> Result<SupportedLanguage, TaskError> {
        match lang.to_lowercase().as_str() {
            "javascript" | "js" => Ok(SupportedLanguage::JavaScript),
            "typescript" | "ts" => Ok(SupportedLanguage::TypeScript),
            "python" | "py" => Ok(SupportedLanguage::Python),
            "rust" | "rs" => Ok(SupportedLanguage::Rust),
            "markdown" | "md" => Ok(SupportedLanguage::Markdown),
            _ => Err(TaskError::InvalidParameter(format!("Unsupported language: {}", lang))),
        }
    }
}

#[async_trait]
impl TaskHandler for TreeSitterTaskHandler {
    async fn handle_task(&self, task: Task, ctx: &TaskContext) -> Result<TaskResult, TaskError> {
        // Deserialize the task request
        let request: TreeSitterTaskRequest = serde_json::from_value(task.params)
            .map_err(|e| TaskError::InvalidParameter(format!("Invalid parameters: {}", e)))?;
        
        match request {
            TreeSitterTaskRequest::ParseFile { file_path } => {
                let path = Path::new(&file_path);
                
                // Read the file content
                let content = ctx.fs.read_to_string(&file_path).await
                    .map_err(|e| TaskError::FileSystem(format!("Failed to read file: {}", e)))?;
                
                // Parse the file
                let result = parse_file(&self.service, path, &content)
                    .map_err(|e| TaskError::from(e))?;
                
                Ok(TaskResult::Json(serde_json::to_value(result).unwrap()))
            },
            
            TreeSitterTaskRequest::SearchDefinitions { file_path } => {
                let path = Path::new(&file_path);
                
                // Read the file content
                let content = ctx.fs.read_to_string(&file_path).await
                    .map_err(|e| TaskError::FileSystem(format!("Failed to read file: {}", e)))?;
                
                // Search for definitions
                let result = search_definitions(&self.service, path, &content)
                    .map_err(|e| TaskError::from(e))?;
                
                Ok(TaskResult::Json(serde_json::to_value(result).unwrap()))
            },
            
            TreeSitterTaskRequest::SearchComponents { file_path } => {
                let path = Path::new(&file_path);
                
                // Read the file content
                let content = ctx.fs.read_to_string(&file_path).await
                    .map_err(|e| TaskError::FileSystem(format!("Failed to read file: {}", e)))?;
                
                // Search for components
                let result = search_components(&self.service, path, &content)
                    .map_err(|e| TaskError::from(e))?;
                
                Ok(TaskResult::Json(serde_json::to_value(result).unwrap()))
            },
            
            TreeSitterTaskRequest::CustomQuery { file_path, query } => {
                let path = Path::new(&file_path);
                
                // Read the file content
                let content = ctx.fs.read_to_string(&file_path).await
                    .map_err(|e| TaskError::FileSystem(format!("Failed to read file: {}", e)))?;
                
                // Run custom query
                let result = run_custom_query(&self.service, path, &content, &query)
                    .map_err(|e| TaskError::from(e))?;
                
                Ok(TaskResult::Json(serde_json::to_value(result).unwrap()))
            },
            
            TreeSitterTaskRequest::ParseString { content, language } => {
                // Parse the language
                let lang = Self::parse_language(&language)?;
                
                // Parse the string
                let result = parse_code_string(&self.service, &content, lang)
                    .map_err(|e| TaskError::from(e))?;
                
                Ok(TaskResult::Json(serde_json::to_value(result).unwrap()))
            },
        }
    }
}
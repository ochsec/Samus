use crate::error::TaskError;
use crate::services::tree_sitter::{
    QueryMatch, SupportedLanguage, TreeSitterError, TreeSitterService,
};
use std::path::Path;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub language: String,
    pub matches: Vec<QueryResult>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct QueryResult {
    pub pattern_index: usize,
    pub captures: Vec<CaptureResult>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CaptureResult {
    pub name: String,
    pub text: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

/// Error types specific to code search operations
#[derive(thiserror::Error, Debug)]
pub enum CodeSearchError {
    #[error("Failed to parse file: {0}")]
    ParseError(String),
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("Invalid query: {0}")]
    QueryError(String),
    #[error("File not found: {0}")]
    FileNotFound(String),
}

impl From<TreeSitterError> for CodeSearchError {
    fn from(err: TreeSitterError) -> Self {
        match err {
            TreeSitterError::UnsupportedLanguage(lang) => Self::UnsupportedLanguage(lang),
            TreeSitterError::QueryError(msg) => Self::QueryError(msg),
            _ => Self::ParseError(err.to_string()),
        }
    }
}

impl From<CodeSearchError> for TaskError {
    fn from(err: CodeSearchError) -> Self {
        TaskError::Tool(err.to_string())
    }
}

/// Convert QueryMatch to our QueryResult format
fn convert_match(query_match: QueryMatch) -> QueryResult {
    QueryResult {
        pattern_index: query_match.pattern_index,
        captures: query_match
            .captures
            .into_iter()
            .map(|c| CaptureResult {
                name: c.name,
                text: c.text,
                start_line: c.start_position.0,
                start_column: c.start_position.1,
                end_line: c.end_position.0,
                end_column: c.end_position.1,
            })
            .collect(),
    }
}

/// Search for definitions (functions, classes, etc.) in a file
pub fn search_definitions(
    service: &TreeSitterService,
    file_path: &Path,
    content: &str,
) -> Result<SearchResult, CodeSearchError> {
    // Extract file extension to determine language
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| CodeSearchError::UnsupportedLanguage("No file extension".to_string()))?;

    let language = SupportedLanguage::from_extension(ext)
        .ok_or_else(|| CodeSearchError::UnsupportedLanguage(ext.to_string()))?;

    // Parse the file
    let tree = service.parse_file(file_path, content)?;
    
    // Get definitions
    let matches = service.get_definitions(language, &tree, content)?;
    
    Ok(SearchResult {
        file_path: file_path.to_string_lossy().to_string(),
        language: format!("{:?}", language),
        matches: matches.into_iter().map(convert_match).collect(),
    })
}

/// Search for components (React components, etc.) in a file
pub fn search_components(
    service: &TreeSitterService,
    file_path: &Path,
    content: &str,
) -> Result<SearchResult, CodeSearchError> {
    // Extract file extension to determine language
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| CodeSearchError::UnsupportedLanguage("No file extension".to_string()))?;

    let language = SupportedLanguage::from_extension(ext)
        .ok_or_else(|| CodeSearchError::UnsupportedLanguage(ext.to_string()))?;

    // Parse the file
    let tree = service.parse_file(file_path, content)?;
    
    // Get components
    let matches = service.get_components(language, &tree, content)?;
    
    Ok(SearchResult {
        file_path: file_path.to_string_lossy().to_string(),
        language: format!("{:?}", language),
        matches: matches.into_iter().map(convert_match).collect(),
    })
}

/// Run a custom query on a file
pub fn run_custom_query(
    service: &TreeSitterService,
    file_path: &Path,
    content: &str,
    query_string: &str,
) -> Result<SearchResult, CodeSearchError> {
    // Extract file extension to determine language
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| CodeSearchError::UnsupportedLanguage("No file extension".to_string()))?;

    let language = SupportedLanguage::from_extension(ext)
        .ok_or_else(|| CodeSearchError::UnsupportedLanguage(ext.to_string()))?;

    // Parse the file
    let tree = service.parse_file(file_path, content)?;
    
    // Run the custom query
    let matches = service.run_query(language, query_string, &tree, content)?;
    
    Ok(SearchResult {
        file_path: file_path.to_string_lossy().to_string(),
        language: format!("{:?}", language),
        matches: matches.into_iter().map(convert_match).collect(),
    })
}
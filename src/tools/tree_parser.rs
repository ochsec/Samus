use crate::error::TaskError;
use crate::services::tree_sitter::{
    SupportedLanguage, Symbol, TreeSitterError, TreeSitterService,
};
use std::path::Path;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ParseFileResult {
    pub file_path: String,
    pub symbols: Vec<Symbol>,
    pub language: String,
}

/// Error types specific to tree parser operations
#[derive(thiserror::Error, Debug)]
pub enum TreeParserError {
    #[error("Failed to parse file: {0}")]
    ParseError(String),
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("File size exceeded limit")]
    FileSizeExceeded,
}

impl From<TreeSitterError> for TreeParserError {
    fn from(err: TreeSitterError) -> Self {
        match err {
            TreeSitterError::UnsupportedLanguage(lang) => Self::UnsupportedLanguage(lang),
            TreeSitterError::FileSizeExceeded => Self::FileSizeExceeded,
            _ => Self::ParseError(err.to_string()),
        }
    }
}

impl From<TreeParserError> for TaskError {
    fn from(err: TreeParserError) -> Self {
        TaskError::Tool(err.to_string())
    }
}

/// Parse a file using tree-sitter and extract symbols (functions, classes, etc.)
pub fn parse_file(
    service: &TreeSitterService,
    file_path: &Path,
    content: &str,
) -> Result<ParseFileResult, TreeParserError> {
    // Extract file extension to determine language
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| TreeParserError::UnsupportedLanguage("No file extension".to_string()))?;

    let language = SupportedLanguage::from_extension(ext)
        .ok_or_else(|| TreeParserError::UnsupportedLanguage(ext.to_string()))?;

    // Find symbols in the file
    let symbols = service.find_symbols(file_path, content)?;

    Ok(ParseFileResult {
        file_path: file_path.to_string_lossy().to_string(),
        symbols,
        language: format!("{:?}", language),
    })
}

/// Parse code definitions from a string with a specified language
pub fn parse_code_string(
    service: &TreeSitterService,
    content: &str,
    language: SupportedLanguage,
) -> Result<Vec<Symbol>, TreeParserError> {
    // Create a temporary file path with the right extension
    let ext = match language {
        SupportedLanguage::JavaScript => "js",
        SupportedLanguage::TypeScript => "ts",
        SupportedLanguage::Python => "py",
        SupportedLanguage::Rust => "rs",
        SupportedLanguage::Markdown => "md",
    };
    
    // Create a longer-lived temporary path
    let temp_path_str = format!("temp.{}", ext);
    let temp_path = Path::new(&temp_path_str);
    
    // Find symbols in the content using the temporary path
    let symbols = service.find_symbols(temp_path, content)?;
    
    Ok(symbols)
}
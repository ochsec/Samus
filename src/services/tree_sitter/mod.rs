use parking_lot::RwLock;
use std::{collections::HashMap, path::Path, sync::Arc};
use thiserror::Error;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

// Module for service initialization
pub mod service_init;

// Re-export service initialization
pub use service_init::initialize_service;

// Error types for tree-sitter operations
#[derive(Error, Debug)]
pub enum TreeSitterError {
    #[error("Language not supported: {0}")]
    UnsupportedLanguage(String),
    #[error("Parser initialization failed: {0}")]
    ParserError(String),
    #[error("File size exceeds limit")]
    FileSizeExceeded,
    #[error("Failed to parse file: {0}")]
    ParseError(String),
    #[error("Query error: {0}")]
    QueryError(String),
}

// Supported languages enum
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SupportedLanguage {
    JavaScript,
    TypeScript,
    Python,
    Rust,
    Markdown,
}

impl SupportedLanguage {
    fn get_language(&self) -> Language {
        match self {
            Self::JavaScript => tree_sitter_javascript::language(),
            Self::TypeScript => tree_sitter_typescript::language_typescript(),
            Self::Python => tree_sitter_python::language(),
            // Temporarily using JavaScript to avoid version issues
            Self::Rust => tree_sitter_javascript::language(),
            // Temporarily using JavaScript to avoid version issues
            Self::Markdown => tree_sitter_javascript::language(),
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "js" => Some(Self::JavaScript),
            "ts" => Some(Self::TypeScript),
            "py" => Some(Self::Python),
            "rs" => Some(Self::Rust),
            "md" | "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }
}

// Parser pool for each language
type ParserPool = Arc<RwLock<Vec<Parser>>>;

pub struct TreeSitterService {
    parser_pools: HashMap<SupportedLanguage, ParserPool>,
    max_file_size: usize,
    max_parsers_per_lang: usize,
}

impl TreeSitterService {
    pub fn new(max_file_size: usize, max_parsers_per_lang: usize) -> Self {
        let mut service = Self {
            parser_pools: HashMap::new(),
            max_file_size,
            max_parsers_per_lang,
        };

        // Initialize parser pools for all supported languages
        for lang in [
            SupportedLanguage::JavaScript,
            SupportedLanguage::TypeScript,
            SupportedLanguage::Python,
            SupportedLanguage::Rust,
            SupportedLanguage::Markdown,
        ] {
            service.init_parser_pool(lang);
        }

        service
    }

    fn init_parser_pool(&mut self, language: SupportedLanguage) {
        let pool = Arc::new(RwLock::new(Vec::with_capacity(self.max_parsers_per_lang)));
        self.parser_pools.insert(language, pool);
    }

    fn get_or_create_parser(&self, language: SupportedLanguage) -> Result<Parser, TreeSitterError> {
        let pool = self
            .parser_pools
            .get(&language)
            .ok_or_else(|| TreeSitterError::UnsupportedLanguage(format!("{:?}", language)))?;

        // Try to get an existing parser from the pool
        if let Some(mut parser) = pool.write().pop() {
            parser.reset();
            return Ok(parser);
        }

        // Create new parser if pool is empty
        let mut parser = Parser::new();
        parser
            .set_language(language.get_language())
            .map_err(|e| TreeSitterError::ParserError(e.to_string()))?;
        Ok(parser)
    }

    fn return_parser(&self, language: SupportedLanguage, parser: Parser) {
        if let Some(pool) = self.parser_pools.get(&language) {
            let mut pool = pool.write();
            if pool.len() < self.max_parsers_per_lang {
                pool.push(parser);
            }
        }
    }

    pub fn parse_file(&self, path: &Path, content: &str) -> Result<Tree, TreeSitterError> {
        if content.len() > self.max_file_size {
            return Err(TreeSitterError::FileSizeExceeded);
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| TreeSitterError::UnsupportedLanguage("No file extension".to_string()))?;

        let language = SupportedLanguage::from_extension(ext)
            .ok_or_else(|| TreeSitterError::UnsupportedLanguage(ext.to_string()))?;

        let mut parser = self.get_or_create_parser(language)?;
        let tree = parser
            .parse(content, None)
            .ok_or_else(|| TreeSitterError::ParseError("Failed to parse content".to_string()))?;

        self.return_parser(language, parser);
        Ok(tree)
    }

    pub fn extract_definitions(&self, tree: &Tree, content: &str) -> Vec<CodeDefinition> {
        let mut definitions = Vec::new();
        let root_node = tree.root_node();

        // Traverse AST and collect definitions
        let mut cursor = root_node.walk();
        self.traverse_definitions(&mut cursor, content, &mut definitions);

        definitions
    }

    fn traverse_definitions(
        &self,
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        definitions: &mut Vec<CodeDefinition>,
    ) {
        loop {
            let node = cursor.node();

            // Check if current node is a definition
            match node.kind() {
                "function_declaration"
                | "class_declaration"
                | "method_definition"
                | "function_item"
                | "struct_item"
                | "impl_item"
                | "class_definition"
                | "function_definition" => {
                    if let Some(name_node) = self.find_definition_name(&node) {
                        definitions.push(CodeDefinition {
                            name: self.get_node_text(name_node, content),
                            kind: node.kind().to_string(),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                        });
                    }
                }
                _ => {}
            }

            // Continue traversing
            if !cursor.goto_first_child() {
                while !cursor.goto_next_sibling() {
                    if !cursor.goto_parent() {
                        return;
                    }
                }
            }
        }
    }

    fn find_definition_name<'a>(
        &self,
        node: &'a tree_sitter::Node,
    ) -> Option<tree_sitter::Node<'a>> {
        let mut cursor = node.walk();
        cursor.goto_first_child();

        loop {
            let current = cursor.node();
            if current.kind() == "identifier" {
                return Some(current);
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        None
    }

    fn get_node_text(&self, node: tree_sitter::Node, content: &str) -> String {
        content[node.byte_range()].to_string()
    }

    // Run a query on a tree and return matches
    pub fn run_query(
        &self,
        language: SupportedLanguage,
        query_str: &str,
        tree: &Tree,
        content: &str,
    ) -> Result<Vec<QueryMatch>, TreeSitterError> {
        let lang = language.get_language();
        let query =
            Query::new(lang, query_str).map_err(|e| TreeSitterError::QueryError(e.to_string()))?;

        let matches = self.execute_query(&query, tree.root_node(), content);
        Ok(matches)
    }

    // Execute a query and get matches
    pub fn execute_query(
        &self,
        query: &Query,
        node: tree_sitter::Node,
        content: &str,
    ) -> Vec<QueryMatch> {
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(query, node, content.as_bytes());

        matches
            .map(|query_match| {
                let captures = query_match
                    .captures
                    .iter()
                    .map(|capture| {
                        let capture_node = capture.node;
                        let capture_name =
                            query.capture_names()[capture.index as usize].to_string();

                        QueryCapture {
                            index: capture.index as usize,
                            name: capture_name,
                            text: self.get_node_text(capture_node, content),
                            start_position: (
                                capture_node.start_position().row + 1,
                                capture_node.start_position().column,
                            ),
                            end_position: (
                                capture_node.end_position().row + 1,
                                capture_node.end_position().column,
                            ),
                        }
                    })
                    .collect();

                QueryMatch {
                    pattern_index: query_match.pattern_index,
                    captures,
                }
            })
            .collect()
    }

    // Use predefined queries from the queries module
    pub fn get_definitions(
        &self,
        language: SupportedLanguage,
        tree: &Tree,
        content: &str,
    ) -> Result<Vec<QueryMatch>, TreeSitterError> {
        let queries = queries::LanguageQueries::get(language)
            .ok_or_else(|| TreeSitterError::UnsupportedLanguage(format!("{:?}", language)))?;

        let matches = self.execute_query(&queries.definitions, tree.root_node(), content);
        Ok(matches)
    }

    // Find components in the code (React components, etc.)
    pub fn get_components(
        &self,
        language: SupportedLanguage,
        tree: &Tree,
        content: &str,
    ) -> Result<Vec<QueryMatch>, TreeSitterError> {
        let queries = queries::LanguageQueries::get(language)
            .ok_or_else(|| TreeSitterError::UnsupportedLanguage(format!("{:?}", language)))?;

        let matches = self.execute_query(&queries.components, tree.root_node(), content);
        Ok(matches)
    }

    // Find all symbols (functions, classes, methods, etc.) in the file
    pub fn find_symbols(&self, path: &Path, content: &str) -> Result<Vec<Symbol>, TreeSitterError> {
        let tree = self.parse_file(path, content)?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| TreeSitterError::UnsupportedLanguage("No file extension".to_string()))?;

        let language = SupportedLanguage::from_extension(ext)
            .ok_or_else(|| TreeSitterError::UnsupportedLanguage(ext.to_string()))?;

        let def_matches = self.get_definitions(language, &tree, content)?;

        // Convert QueryMatch to Symbol
        let symbols = def_matches
            .into_iter()
            .filter_map(|m| {
                let name_capture = m.captures.iter().find(|c| c.name.contains(".name"));

                let kind_str = m
                    .captures
                    .iter()
                    .find(|c| c.name.ends_with(".definition"))
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                name_capture.map(|nc| Symbol {
                    name: nc.text.clone(),
                    kind: self.determine_symbol_kind(&kind_str),
                    start_line: nc.start_position.0,
                    end_line: nc.end_position.0,
                })
            })
            .collect();

        Ok(symbols)
    }

    // Helper method to determine symbol kind
    fn determine_symbol_kind(&self, capture_name: &str) -> SymbolKind {
        if capture_name.contains("function") {
            SymbolKind::Function
        } else if capture_name.contains("class") {
            SymbolKind::Class
        } else if capture_name.contains("method") {
            SymbolKind::Method
        } else if capture_name.contains("struct") {
            SymbolKind::Struct
        } else if capture_name.contains("interface") {
            SymbolKind::Interface
        } else if capture_name.contains("component") {
            SymbolKind::Component
        } else if capture_name.contains("impl") {
            SymbolKind::Implementation
        } else if capture_name.contains("module") || capture_name.contains("mod") {
            SymbolKind::Module
        } else {
            SymbolKind::Other
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeDefinition {
    pub name: String,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryMatch {
    pub pattern_index: usize,
    pub captures: Vec<QueryCapture>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryCapture {
    pub index: usize,
    pub name: String,
    pub text: String,
    pub start_position: (usize, usize), // (line, column)
    pub end_position: (usize, usize),   // (line, column)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Struct,
    Interface,
    Component,
    Implementation,
    Module,
    Other,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
}

// Module for language-specific queries
pub mod queries;

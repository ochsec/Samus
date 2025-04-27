mod apply_diff;
mod read_file;
mod tree_parser;
mod code_search;

pub use tree_parser::{TreeParserError, parse_file, parse_code_string};
pub use code_search::{CodeSearchError, 
                     search_definitions, search_components, run_custom_query};

// Re-export core tool types and functions
pub type Result<T> = std::result::Result<T, crate::error::TaskError>;

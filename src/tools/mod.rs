mod apply_diff;
mod read_file;
mod tree_parser;
mod code_search;

pub use apply_diff::{ApplyDiffError, ApplyDiffResult, apply_diff};
pub use read_file::{FileError, FileStats, ReadFileResult, read_file_with_lines};
pub use tree_parser::{ParseFileResult, TreeParserError, parse_file, parse_code_string};
pub use code_search::{SearchResult, QueryResult, CaptureResult, CodeSearchError, 
                     search_definitions, search_components, run_custom_query};

// Re-export core tool types and functions
pub type Result<T> = std::result::Result<T, crate::error::TaskError>;

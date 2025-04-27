use super::TreeSitterService;
use crate::config::Config;
use std::sync::Arc;

const DEFAULT_MAX_FILE_SIZE: usize = 5 * 1024 * 1024; // 5 MB
const DEFAULT_MAX_PARSERS_PER_LANG: usize = 4;

pub fn initialize_service(config: &Config) -> Arc<TreeSitterService> {
    // Extract configuration values or use defaults
    let max_file_size = config
        .get_usize("tree_sitter.max_file_size")
        .unwrap_or(DEFAULT_MAX_FILE_SIZE);
    
    let max_parsers_per_lang = config
        .get_usize("tree_sitter.max_parsers_per_lang")
        .unwrap_or(DEFAULT_MAX_PARSERS_PER_LANG);
    
    // Create the service with the configured values
    let service = TreeSitterService::new(max_file_size, max_parsers_per_lang);
    
    Arc::new(service)
}
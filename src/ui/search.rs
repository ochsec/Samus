use dashmap::DashMap;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use regex::Regex;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Maximum number of queries to keep in history
const MAX_QUERY_HISTORY: usize = 50;

/// Search result with context and highlighting information
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub line_number: usize,
    pub line_content: String,
    pub start_pos: usize,
    pub length: usize,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub source_id: String,
    pub source_type: String,
    pub current_match: usize, // Current position in matches for navigation
}

impl SearchResult {
    pub fn new(matches: Vec<SearchMatch>, source_id: String, source_type: String) -> Self {
        Self {
            matches,
            source_id,
            source_type,
            current_match: 0,
        }
    }

    /// Navigate to the next match in the results
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    /// Navigate to the previous match in the results
    pub fn previous_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = self
                .current_match
                .checked_sub(1)
                .unwrap_or(self.matches.len() - 1);
        }
    }

    /// Get the current match
    pub fn current(&self) -> Option<&SearchMatch> {
        self.matches.get(self.current_match)
    }
}

/// Search options for customizing search behavior
#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub context_lines: usize,
    pub regex_mode: bool,
    pub fuzzy_threshold: i64,
    pub whole_word: bool,    // New: Match whole words only
    pub highlight_all: bool, // New: Highlight all matches in line
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            context_lines: 2,
            regex_mode: false,
            fuzzy_threshold: 50,
            whole_word: false,
            highlight_all: true,
        }
    }
}

/// Search engine trait defining the interface for different search implementations
#[async_trait::async_trait]
pub trait SearchEngine: Send + Sync {
    async fn search(&self, text: &str, query: &str, options: &SearchOptions) -> Vec<SearchMatch>;
    async fn update_index(&self, id: String, content: String);
    async fn clear_index(&self);
}

// Regex-based search implementation
#[derive(Debug)]
pub struct RegexSearch {
    index: Arc<DashMap<String, String>>,
}

impl RegexSearch {
    pub fn new() -> Self {
        Self {
            index: Arc::new(DashMap::new()),
        }
    }

    fn create_regex(&self, query: &str, options: &SearchOptions) -> Result<Regex, regex::Error> {
        let mut pattern = String::new();

        if !options.case_sensitive {
            pattern.push_str("(?i)");
        }

        if options.whole_word {
            pattern.push_str("\\b");
        }

        pattern.push_str(query);

        if options.whole_word {
            pattern.push_str("\\b");
        }

        Regex::new(&pattern)
    }
}

#[async_trait::async_trait]
impl SearchEngine for RegexSearch {
    async fn search(&self, text: &str, query: &str, options: &SearchOptions) -> Vec<SearchMatch> {
        let regex = match self.create_regex(query, options) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        let mut matches = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        for (line_idx, &line) in lines.iter().enumerate() {
            let mut line_matches = Vec::new();
            for captures in regex.captures_iter(line) {
                let m = captures.get(0).unwrap();
                line_matches.push((m.start(), m.end()));
            }

            if !line_matches.is_empty() {
                let before_start = line_idx.saturating_sub(options.context_lines);
                let after_end = (line_idx + options.context_lines + 1).min(lines.len());

                let context_before: Vec<String> = lines[before_start..line_idx]
                    .iter()
                    .map(|&s| s.to_string())
                    .collect();

                let context_after: Vec<String> = lines[(line_idx + 1)..after_end]
                    .iter()
                    .map(|&s| s.to_string())
                    .collect();

                // Create a match for each occurrence if highlight_all is true
                if options.highlight_all {
                    for (start, end) in line_matches {
                        matches.push(SearchMatch {
                            line_number: line_idx + 1,
                            line_content: line.to_string(),
                            start_pos: start,
                            length: end - start,
                            context_before: context_before.clone(),
                            context_after: context_after.clone(),
                        });
                    }
                } else {
                    // Only use the first match if highlight_all is false
                    let (start, end) = line_matches[0];
                    matches.push(SearchMatch {
                        line_number: line_idx + 1,
                        line_content: line.to_string(),
                        start_pos: start,
                        length: end - start,
                        context_before,
                        context_after,
                    });
                }
            }
        }

        matches
    }

    async fn update_index(&self, id: String, content: String) {
        self.index.insert(id, content);
    }

    async fn clear_index(&self) {
        self.index.clear();
    }
}

// Enhanced fuzzy search implementation
pub struct FuzzySearch {
    index: Arc<DashMap<String, String>>,
    matcher: Arc<SkimMatcherV2>,
}

impl std::fmt::Debug for FuzzySearch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FuzzySearch")
            .field("index", &self.index)
            .field("matcher", &"SkimMatcherV2")
            .finish()
    }
}

impl FuzzySearch {
    pub fn new() -> Self {
        Self {
            index: Arc::new(DashMap::new()),
            matcher: Arc::new(SkimMatcherV2::default()),
        }
    }
}

#[async_trait::async_trait]
impl SearchEngine for FuzzySearch {
    async fn search(&self, text: &str, query: &str, options: &SearchOptions) -> Vec<SearchMatch> {
        let mut matches = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        for (line_idx, &line) in lines.iter().enumerate() {
            if let Some((score, indices)) = self.matcher.fuzzy_indices(line, query) {
                if score >= options.fuzzy_threshold {
                    let before_start = line_idx.saturating_sub(options.context_lines);
                    let after_end = (line_idx + options.context_lines + 1).min(lines.len());

                    let context_before = lines[before_start..line_idx]
                        .iter()
                        .map(|&s| s.to_string())
                        .collect();

                    let context_after: Vec<String> = lines[(line_idx + 1)..after_end]
                        .iter()
                        .map(|&s| s.to_string())
                        .collect();

                    // For fuzzy search, we highlight the matched characters
                    let start_pos = indices.first().copied().unwrap_or(0);
                    let length = indices.last().map(|&i| i - start_pos + 1).unwrap_or(0);

                    matches.push(SearchMatch {
                        line_number: line_idx + 1,
                        line_content: line.to_string(),
                        start_pos,
                        length,
                        context_before,
                        context_after,
                    });
                }
            }
        }

        matches
    }

    async fn update_index(&self, id: String, content: String) {
        self.index.insert(id, content);
    }

    async fn clear_index(&self) {
        self.index.clear();
    }
}

/// Search manager that coordinates different search engines and maintains the search state
#[derive(Debug)]
pub struct SearchManager {
    regex_engine: Arc<RegexSearch>,
    fuzzy_engine: Arc<FuzzySearch>,
    options: Arc<RwLock<SearchOptions>>,
    query_history: Arc<RwLock<VecDeque<String>>>,
    current_result: Arc<RwLock<Option<SearchResult>>>,
}

impl SearchManager {
    pub fn new() -> Self {
        Self {
            regex_engine: Arc::new(RegexSearch::new()),
            fuzzy_engine: Arc::new(FuzzySearch::new()),
            options: Arc::new(RwLock::new(SearchOptions::default())),
            query_history: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_QUERY_HISTORY))),
            current_result: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn search(&self, text: &str, query: &str) -> SearchResult {
        // Add query to history
        self.add_to_history(query.to_string()).await;

        let options = self.options.read().await;
        let matches = if options.regex_mode {
            self.regex_engine.search(text, query, &options).await
        } else {
            self.fuzzy_engine.search(text, query, &options).await
        };

        let result = SearchResult::new(matches, String::new(), String::new());
        *self.current_result.write().await = Some(result.clone());
        result
    }

    pub async fn update_index(&self, id: String, content: String) {
        let content_clone = content.clone();
        self.regex_engine.update_index(id.clone(), content).await;
        self.fuzzy_engine.update_index(id, content_clone).await;
    }

    pub async fn clear_index(&self) {
        self.regex_engine.clear_index().await;
        self.fuzzy_engine.clear_index().await;
    }

    pub async fn set_options(&self, options: SearchOptions) {
        *self.options.write().await = options;
    }

    pub async fn get_options(&self) -> SearchOptions {
        self.options.read().await.clone()
    }

    /// Add a query to the search history
    async fn add_to_history(&self, query: String) {
        let mut history = self.query_history.write().await;
        if !history.contains(&query) {
            if history.len() >= MAX_QUERY_HISTORY {
                history.pop_back();
            }
            history.push_front(query);
        }
    }

    /// Get the search history
    pub async fn get_history(&self) -> Vec<String> {
        self.query_history.read().await.iter().cloned().collect()
    }

    /// Navigate to next match in current search result
    pub async fn next_match(&self) {
        if let Some(mut result) = self.current_result.write().await.take() {
            result.next_match();
            *self.current_result.write().await = Some(result);
        }
    }

    /// Navigate to previous match in current search result
    pub async fn previous_match(&self) {
        if let Some(mut result) = self.current_result.write().await.take() {
            result.previous_match();
            *self.current_result.write().await = Some(result);
        }
    }

    /// Get the current search result
    pub async fn get_current_result(&self) -> Option<SearchResult> {
        self.current_result.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_regex_search() {
        let engine = RegexSearch::new();
        let text = "Hello World\nTest Line\nHello Test";
        let options = SearchOptions::default();

        let results = engine.search(text, "Hello", &options).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].line_number, 1);
        assert_eq!(results[1].line_number, 3);
    }

    #[tokio::test]
    async fn test_fuzzy_search() {
        let engine = FuzzySearch::new();
        let text = "Hello World\nTest Line\nHello Test";
        let options = SearchOptions::default();

        let results = engine.search(text, "Helo", &options).await;
        assert!(!results.is_empty());
        assert!(results.iter().any(|m| m.line_content.contains("Hello")));
    }

    #[tokio::test]
    async fn test_search_manager() {
        let manager = SearchManager::new();
        let text = "Hello World\nTest Line\nHello Test";

        // Test regex search
        manager
            .set_options(SearchOptions {
                regex_mode: true,
                ..Default::default()
            })
            .await;
        let results = manager.search(text, "Hello").await;
        assert_eq!(results.matches.len(), 2);

        // Test fuzzy search
        manager
            .set_options(SearchOptions {
                regex_mode: false,
                ..Default::default()
            })
            .await;
        let results = manager.search(text, "Helo").await;
        assert!(!results.matches.is_empty());
    }

    #[tokio::test]
    async fn test_search_history() {
        let manager = SearchManager::new();
        let text = "Hello World";

        // Add some searches to history
        manager.search(text, "Hello").await;
        manager.search(text, "World").await;

        let history = manager.get_history().await;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], "World");
        assert_eq!(history[1], "Hello");
    }

    #[tokio::test]
    async fn test_search_navigation() {
        let manager = SearchManager::new();
        let text = "Hello World\nHello Test\nHello Again";

        let result = manager.search(text, "Hello").await;
        assert_eq!(result.current_match, 0);

        manager.next_match().await;
        let result = manager.get_current_result().await.unwrap();
        assert_eq!(result.current_match, 1);

        manager.previous_match().await;
        let result = manager.get_current_result().await.unwrap();
        assert_eq!(result.current_match, 0);
    }
}

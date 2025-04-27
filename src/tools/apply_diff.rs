use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use similar::{ChangeTag, TextDiff};
use thiserror::Error;

use crate::{
    error::TaskError,
    ui::diff::{DiffConfig, DiffVisualization},
};

const BACKUP_EXTENSION: &str = ".bak";

#[derive(Debug, Error)]
pub enum ApplyDiffError {
    #[error("File not found: {0}")]
    NotFound(PathBuf),

    #[error("File is outside workspace boundaries")]
    OutsideWorkspace,

    #[error("Invalid diff format: {0}")]
    InvalidDiffFormat(String),

    #[error("Failed to parse line number: {0}")]
    LineNumberParse(String),

    #[error("Content mismatch between file and diff")]
    ContentMismatch,

    #[error("Failed to create backup: {0}")]
    BackupFailed(String),

    #[error("Failed to update file: {0}")]
    UpdateFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

impl From<ApplyDiffError> for TaskError {
    fn from(err: ApplyDiffError) -> Self {
        match err {
            ApplyDiffError::IoError(e) => TaskError::IoError(e),
            _ => TaskError::ExecutionFailed(err.to_string()),
        }
    }
}

#[derive(Debug)]
struct DiffBlock {
    start_line: usize,
    original: String,
    replacement: String,
}

pub struct ApplyDiffResult {
    pub preview: DiffVisualization,
    pub changes_applied: bool,
    pub backup_path: Option<PathBuf>,
}

pub fn parse_diff_block(content: &str) -> Result<DiffBlock, ApplyDiffError> {
    let parts: Vec<&str> = content.split("=======").collect();
    if parts.len() != 2 {
        return Err(ApplyDiffError::InvalidDiffFormat(
            "Missing separator".to_string(),
        ));
    }

    let search_part = parts[0];
    let replace_part = parts[1];

    // Parse start line
    let start_line_str = search_part
        .lines()
        .find(|line| line.starts_with(":start_line:"))
        .ok_or_else(|| ApplyDiffError::InvalidDiffFormat("Missing start line".to_string()))?;

    let start_line: usize = start_line_str
        .trim_start_matches(":start_line:")
        .trim()
        .parse()
        .map_err(|_| ApplyDiffError::LineNumberParse(start_line_str.to_string()))?;

    // Extract original and replacement content
    let original = search_part
        .lines()
        .skip_while(|line| !line.starts_with("-------"))
        .skip(1)
        .collect::<Vec<&str>>()
        .join("\n");

    let replacement = replace_part
        .lines()
        .take_while(|line| !line.starts_with(">>>>>>> REPLACE"))
        .collect::<Vec<&str>>()
        .join("\n");

    Ok(DiffBlock {
        start_line,
        original: original.trim().to_string(),
        replacement: replacement.trim().to_string(),
    })
}

pub fn validate_path(
    path: impl AsRef<Path>,
    workspace_root: impl AsRef<Path>,
) -> Result<PathBuf, ApplyDiffError> {
    let path = path.as_ref();
    let canonical_path = path
        .canonicalize()
        .map_err(|_| ApplyDiffError::NotFound(path.to_path_buf()))?;
    let workspace_root = workspace_root
        .as_ref()
        .canonicalize()
        .map_err(|_| ApplyDiffError::OutsideWorkspace)?;

    if !canonical_path.starts_with(workspace_root) {
        return Err(ApplyDiffError::OutsideWorkspace);
    }

    Ok(canonical_path)
}

pub fn create_backup(path: &Path) -> Result<PathBuf, ApplyDiffError> {
    let extension_string = path.extension()
        .map(|ext| format!("{}.{}", ext.to_string_lossy(), BACKUP_EXTENSION))
        .unwrap_or_else(|| BACKUP_EXTENSION.to_string());
        
    let backup_path = path.with_extension(extension_string);

    fs::copy(path, &backup_path).map_err(|e| ApplyDiffError::BackupFailed(e.to_string()))?;
    Ok(backup_path)
}

pub fn apply_diff(
    path: impl AsRef<Path>,
    workspace_root: impl AsRef<Path>,
    diff_content: &str,
) -> Result<ApplyDiffResult, ApplyDiffError> {
    let canonical_path = validate_path(&path, workspace_root)?;

    // Read original file content
    let mut file = File::open(&canonical_path)?;
    let mut original_content = String::new();
    file.read_to_string(&mut original_content)?;

    // Parse diff blocks
    let blocks: Vec<DiffBlock> = diff_content
        .split("<<<<<<< SEARCH")
        .filter(|block| !block.trim().is_empty())
        .map(parse_diff_block)
        .collect::<Result<Vec<_>, _>>()?;

    // Validate content matches
    for block in &blocks {
        let original_lines: Vec<&str> = original_content.lines().collect();
        if block.start_line > original_lines.len() {
            return Err(ApplyDiffError::ContentMismatch);
        }

        let chunk_lines: Vec<&str> = block.original.lines().collect();
        let file_chunk: Vec<&str> = original_lines
            .iter()
            .skip(block.start_line - 1)
            .take(chunk_lines.len())
            .copied()
            .collect();

        if chunk_lines != file_chunk {
            return Err(ApplyDiffError::ContentMismatch);
        }
    }

    // Create diff visualization for preview
    let mut new_content = original_content.clone();
    for block in &blocks {
        let lines: Vec<&str> = new_content.lines().collect();
        let prefix = lines[..block.start_line - 1].join("\n");
        let suffix = lines[block.start_line - 1 + block.original.lines().count()..].join("\n");

        new_content = if prefix.is_empty() && suffix.is_empty() {
            block.replacement.clone()
        } else if prefix.is_empty() {
            format!("{}\n{}", block.replacement, suffix)
        } else if suffix.is_empty() {
            format!("{}\n{}", prefix, block.replacement)
        } else {
            format!("{}\n{}\n{}", prefix, block.replacement, suffix)
        };
    }

    let preview = DiffVisualization::new(original_content.clone(), new_content.clone())
        .with_config(DiffConfig::default());

    // Create backup and apply changes
    let backup_path = create_backup(&canonical_path)?;

    // Write updated content
    let mut file =
        File::create(&canonical_path).map_err(|e| ApplyDiffError::UpdateFailed(e.to_string()))?;
    file.write_all(new_content.as_bytes())
        .map_err(|e| ApplyDiffError::UpdateFailed(e.to_string()))?;

    Ok(ApplyDiffResult {
        preview,
        changes_applied: true,
        backup_path: Some(backup_path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_diff_block() {
        let diff_content = r#"<<<<<<< SEARCH
:start_line:1
-------
original content
=======
new content
>>>>>>> REPLACE"#;

        let block = parse_diff_block(diff_content).unwrap();
        assert_eq!(block.start_line, 1);
        assert_eq!(block.original, "original content");
        assert_eq!(block.replacement, "new content");
    }

    #[test]
    fn test_apply_diff_basic() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        write(&test_file, "line 1\nline 2\nline 3").unwrap();

        let diff_content = r#"<<<<<<< SEARCH
:start_line:2
-------
line 2
=======
updated line 2
>>>>>>> REPLACE"#;

        let result = apply_diff(&test_file, temp.path(), diff_content).unwrap();
        assert!(result.changes_applied);
        assert!(result.backup_path.is_some());

        let updated_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(updated_content, "line 1\nupdated line 2\nline 3");
    }

    #[test]
    fn test_apply_diff_content_mismatch() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        write(&test_file, "line 1\nline 2\nline 3").unwrap();

        let diff_content = r#"<<<<<<< SEARCH
:start_line:2
-------
wrong content
=======
updated line 2
>>>>>>> REPLACE"#;

        let result = apply_diff(&test_file, temp.path(), diff_content);
        assert!(matches!(result, Err(ApplyDiffError::ContentMismatch)));
    }

    #[test]
    fn test_apply_diff_multiple_blocks() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        write(&test_file, "line 1\nline 2\nline 3\nline 4").unwrap();

        let diff_content = r#"<<<<<<< SEARCH
:start_line:1
-------
line 1
=======
updated line 1
>>>>>>> REPLACE

<<<<<<< SEARCH
:start_line:3
-------
line 3
=======
updated line 3
>>>>>>> REPLACE"#;

        let result = apply_diff(&test_file, temp.path(), diff_content).unwrap();
        assert!(result.changes_applied);

        let updated_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(
            updated_content,
            "updated line 1\nline 2\nupdated line 3\nline 4"
        );
    }
}

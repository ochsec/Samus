use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use crate::error::TaskError;
use metrics::counter;

const MAX_READ_FILE_LINES: usize = 10000;
const BINARY_CHECK_SIZE: usize = 8000;

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("File not found: {0}")]
    NotFound(PathBuf),

    #[error("File is not within workspace boundaries")]
    OutsideWorkspace,

    #[error("File appears to be binary")]
    BinaryFile,

    #[error("Invalid line range: start_line ({start}) greater than end_line ({end})")]
    InvalidLineRange { start: usize, end: usize },

    #[error("Line number out of bounds: {requested} exceeds {total} total lines")]
    LineOutOfBounds { requested: usize, total: usize },

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

impl From<FileError> for TaskError {
    fn from(err: FileError) -> Self {
        match err {
            FileError::IoError(e) => TaskError::IoError(e),
            _ => TaskError::ExecutionFailed(err.to_string()),
        }
    }
}

pub struct FileStats {
    pub total_lines: usize,
    pub is_truncated: bool,
    pub is_binary: bool,
}

pub struct ReadFileResult {
    pub content: String,
    pub stats: FileStats,
}

/// Checks if a file appears to be binary by examining its first N bytes
fn is_binary_file(mut file: &File) -> io::Result<bool> {
    let mut buffer = vec![0; BINARY_CHECK_SIZE];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    // Reset file position
    file.seek(SeekFrom::Start(0))?;

    Ok(buffer
        .iter()
        .any(|&byte| byte == 0 || (byte < 32 && byte != b'\n' && byte != b'\r' && byte != b'\t')))
}

fn validate_line_range(
    start_line: Option<usize>,
    end_line: Option<usize>,
    total_lines: usize,
) -> Result<(usize, usize), FileError> {
    let start = start_line.unwrap_or(1);
    let end = end_line.unwrap_or(total_lines);

    if start == 0 {
        return Err(FileError::InvalidLineRange { start, end });
    }

    if start > end {
        return Err(FileError::InvalidLineRange { start, end });
    }

    if start > total_lines {
        return Err(FileError::LineOutOfBounds {
            requested: start,
            total: total_lines,
        });
    }

    Ok((start, end.min(total_lines)))
}

/// Counts total lines in a file efficiently
fn count_lines(file: &mut File) -> io::Result<usize> {
    let mut count = 0;
    let mut buffer = [0; 16384];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        count += buffer[..bytes_read].iter().filter(|&&b| b == b'\n').count();
    }

    // Reset file position
    file.seek(SeekFrom::Start(0))?;
    Ok(count + 1)
}

pub fn read_file_with_lines(
    path: impl AsRef<Path>,
    workspace_root: impl AsRef<Path>,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<ReadFileResult, FileError> {
    let path = path.as_ref();

    // Validate path is within workspace
    let canonical_path = path
        .canonicalize()
        .map_err(|_| FileError::NotFound(path.to_path_buf()))?;
    let workspace_root = workspace_root.as_ref().canonicalize()?;

    if !canonical_path.starts_with(workspace_root) {
        return Err(FileError::OutsideWorkspace);
    }

    // Open and check if binary
    let mut file = File::open(path).map_err(|_| FileError::NotFound(path.to_path_buf()))?;
    let is_binary = is_binary_file(&file)?;

    if is_binary {
        return Err(FileError::BinaryFile);
    }

    // Count total lines
    let total_lines = count_lines(&mut file)?;

    // Validate line range
    let (start, end) = validate_line_range(start_line, end_line, total_lines)?;

    // Read requested lines
    let reader = BufReader::new(file);
    let mut content = String::new();
    let mut current_line = 0;

    for (idx, line) in reader.lines().enumerate() {
        current_line = idx + 1;

        if current_line >= start {
            if current_line > end || current_line - start >= MAX_READ_FILE_LINES {
                break;
            }
            let line = line?;
            content.push_str(&format!("{} | {}\n", current_line, line));
        }
    }

    let stats = FileStats {
        total_lines,
        is_truncated: current_line < end,
        is_binary: false,
    };

    if stats.is_truncated {}

    Ok(ReadFileResult { content, stats })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::TempDir;

    #[test]
    fn test_read_file_basic() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        write(&test_file, "line 1\nline 2\nline 3").unwrap();

        let result = read_file_with_lines(&test_file, temp.path(), None, None).unwrap();
        assert_eq!(result.stats.total_lines, 3);
        assert!(!result.stats.is_truncated);
        assert!(!result.stats.is_binary);
    }

    #[test]
    fn test_read_file_line_range() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        write(&test_file, "line 1\nline 2\nline 3\nline 4").unwrap();

        let result = read_file_with_lines(&test_file, temp.path(), Some(2), Some(3)).unwrap();
        assert!(result.content.contains("2 | line 2"));
        assert!(result.content.contains("3 | line 3"));
        assert!(!result.content.contains("1 | line 1"));
        assert!(!result.content.contains("4 | line 4"));
    }

    #[test]
    fn test_binary_file_detection() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("binary.dat");
        write(&test_file, &[0, 1, 2, 3, 0, 5, 6, 7]).unwrap();

        let result = read_file_with_lines(&test_file, temp.path(), None, None);
        assert!(matches!(result, Err(FileError::BinaryFile)));
    }
}

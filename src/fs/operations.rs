use crate::error::TaskError;
use async_trait::async_trait;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(100);

/// Normalizes a path to use platform-specific separators and resolves relative components
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push("/"),
            Component::Normal(name) => normalized.push(name),
            Component::CurDir => {} // Skip .
            Component::ParentDir => {
                // Handle ..
                normalized.pop();
            }
        }
    }
    normalized
}

/// Retry a fallible operation with exponential backoff
async fn retry_operation<F, T>(mut operation: F) -> Result<T, TaskError>
where
    F: FnMut() -> Result<T, TaskError>,
{
    let mut retries = 0;
    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(err) => {
                if retries >= MAX_RETRIES {
                    return Err(err);
                }
                if let TaskError::IoError(io_err) = &err {
                    match io_err.kind() {
                        ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::TimedOut => {
                            tokio::time::sleep(RETRY_DELAY * 2_u32.pow(retries)).await;
                            retries += 1;
                            continue;
                        }
                        _ => return Err(err),
                    }
                }
                return Err(err);
            }
        }
    }
}

/// Trait for filesystem operations.
#[async_trait]
pub trait FileSystem: Send + Sync {
    /// Check if a file exists.
    async fn file_exists(&self, path: &str) -> bool;

    /// Read a file as a string.
    async fn read_to_string(&self, path: &str) -> Result<String, TaskError>;

    /// Write a string to a file.
    async fn write_to_file(&self, path: &str, content: &str) -> Result<(), TaskError>;

    /// List files in a directory recursively.
    async fn list_files(&self, dir: &str) -> Result<Vec<String>, TaskError>;

    /// Create a directory and any necessary parent directories.
    async fn create_dir(&self, path: &str) -> Result<(), TaskError>;

    /// Delete a file.
    async fn delete_file(&self, path: &str) -> Result<(), TaskError>;

    /// Delete a directory and all its contents.
    async fn delete_dir(&self, path: &str) -> Result<(), TaskError>;

    /// Rename/move a file.
    async fn rename_file(&self, from: &str, to: &str) -> Result<(), TaskError>;

    /// Copy a file.
    async fn copy_file(&self, from: &str, to: &str) -> Result<(), TaskError>;

    /// Get file metadata (size, timestamps, etc).
    async fn file_metadata(&self, path: &str) -> Result<fs::Metadata, TaskError>;
}

/// Concrete implementation of FileSystem.
pub struct LocalFileSystem;

impl LocalFileSystem {
    pub fn new() -> Self {
        LocalFileSystem
    }
}

impl Default for LocalFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystem for LocalFileSystem {
    async fn file_exists(&self, path: &str) -> bool {
        let path = Path::new(path);
        let normalized = normalize_path(path);
        normalized.exists()
    }

    async fn read_to_string(&self, path: &str) -> Result<String, TaskError> {
        let path = Path::new(path);
        let normalized = normalize_path(path);
        retry_operation(|| {
            let mut file = File::open(&normalized).map_err(TaskError::from)?;
            let mut content = String::new();
            file.read_to_string(&mut content).map_err(TaskError::from)?;
            Ok(content)
        })
        .await
    }

    async fn write_to_file(&self, path: &str, content: &str) -> Result<(), TaskError> {
        let path = Path::new(path);
        let normalized = normalize_path(path);
        if let Some(parent) = normalized.parent() {
            fs::create_dir_all(parent).map_err(TaskError::from)?;
        }
        retry_operation(|| {
            let mut file = File::create(&normalized).map_err(TaskError::from)?;
            file.write_all(content.as_bytes()).map_err(TaskError::from)
        })
        .await
    }

    async fn list_files(&self, dir: &str) -> Result<Vec<String>, TaskError> {
        let path = Path::new(dir);
        let normalized = normalize_path(path);
        retry_operation(|| {
            let mut files = Vec::new();
            visit_dirs(&normalized, &mut files)?;
            let string_files = files.iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            Ok(string_files)
        })
        .await
    }

    async fn create_dir(&self, path: &str) -> Result<(), TaskError> {
        let path = Path::new(path);
        let normalized = normalize_path(path);
        retry_operation(|| fs::create_dir_all(&normalized).map_err(TaskError::from)).await
    }

    async fn delete_file(&self, path: &str) -> Result<(), TaskError> {
        let path = Path::new(path);
        let normalized = normalize_path(path);
        retry_operation(|| fs::remove_file(&normalized).map_err(TaskError::from)).await
    }

    async fn delete_dir(&self, path: &str) -> Result<(), TaskError> {
        let path = Path::new(path);
        let normalized = normalize_path(path);
        retry_operation(|| fs::remove_dir_all(&normalized).map_err(TaskError::from)).await
    }

    async fn rename_file(&self, from: &str, to: &str) -> Result<(), TaskError> {
        let from_path = Path::new(from);
        let to_path = Path::new(to);
        let from_norm = normalize_path(from_path);
        let to_norm = normalize_path(to_path);
        retry_operation(|| fs::rename(&from_norm, &to_norm).map_err(TaskError::from)).await
    }

    async fn copy_file(&self, from: &str, to: &str) -> Result<(), TaskError> {
        let from_path = Path::new(from);
        let to_path = Path::new(to);
        let from_norm = normalize_path(from_path);
        let to_norm = normalize_path(to_path);
        retry_operation(|| {
            fs::copy(&from_norm, &to_norm).map_err(TaskError::from)?;
            Ok(())
        })
        .await
    }

    async fn file_metadata(&self, path: &str) -> Result<fs::Metadata, TaskError> {
        let path = Path::new(path);
        let normalized = normalize_path(path);
        retry_operation(|| fs::metadata(&normalized).map_err(TaskError::from)).await
    }
}

/// Legacy trait for compatibility
#[async_trait]
pub trait FileSystemOperations: Send + Sync {
    /// Check if a file exists.
    async fn file_exists(&self, path: &Path) -> bool;

    /// Read a file as a string.
    async fn read_file(&self, path: &Path) -> Result<String, TaskError>;

    /// Write a string to a file.
    async fn write_file(&self, path: &Path, content: &str) -> Result<(), TaskError>;

    /// List files in a directory recursively.
    async fn list_files(&self, dir: &Path) -> Result<Vec<PathBuf>, TaskError>;

    /// Create a directory and any necessary parent directories.
    async fn create_dir(&self, path: &Path) -> Result<(), TaskError>;

    /// Delete a file.
    async fn delete_file(&self, path: &Path) -> Result<(), TaskError>;

    /// Delete a directory and all its contents.
    async fn delete_dir(&self, path: &Path) -> Result<(), TaskError>;

    /// Rename/move a file.
    async fn rename_file(&self, from: &Path, to: &Path) -> Result<(), TaskError>;

    /// Copy a file.
    async fn copy_file(&self, from: &Path, to: &Path) -> Result<(), TaskError>;

    /// Get file metadata (size, timestamps, etc).
    async fn file_metadata(&self, path: &Path) -> Result<fs::Metadata, TaskError>;
}

/// Legacy implementation for compatibility
pub struct FileSystemOperationsImpl;

impl FileSystemOperationsImpl {
    pub fn new() -> Self {
        FileSystemOperationsImpl
    }
}

impl Default for FileSystemOperationsImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystemOperations for FileSystemOperationsImpl {
    async fn file_exists(&self, path: &Path) -> bool {
        let normalized = normalize_path(path);
        normalized.exists()
    }

    async fn read_file(&self, path: &Path) -> Result<String, TaskError> {
        let normalized = normalize_path(path);
        retry_operation(|| {
            let mut file = File::open(&normalized).map_err(TaskError::from)?;
            let mut content = String::new();
            file.read_to_string(&mut content).map_err(TaskError::from)?;
            Ok(content)
        })
        .await
    }

    async fn write_file(&self, path: &Path, content: &str) -> Result<(), TaskError> {
        let normalized = normalize_path(path);
        if let Some(parent) = normalized.parent() {
            fs::create_dir_all(parent).map_err(TaskError::from)?;
        }
        retry_operation(|| {
            let mut file = File::create(&normalized).map_err(TaskError::from)?;
            file.write_all(content.as_bytes()).map_err(TaskError::from)
        })
        .await
    }

    async fn list_files(&self, dir: &Path) -> Result<Vec<PathBuf>, TaskError> {
        let normalized = normalize_path(dir);
        retry_operation(|| {
            let mut files = Vec::new();
            visit_dirs(&normalized, &mut files)?;
            Ok(files)
        })
        .await
    }

    async fn create_dir(&self, path: &Path) -> Result<(), TaskError> {
        let normalized = normalize_path(path);
        retry_operation(|| fs::create_dir_all(&normalized).map_err(TaskError::from)).await
    }

    async fn delete_file(&self, path: &Path) -> Result<(), TaskError> {
        let normalized = normalize_path(path);
        retry_operation(|| fs::remove_file(&normalized).map_err(TaskError::from)).await
    }

    async fn delete_dir(&self, path: &Path) -> Result<(), TaskError> {
        let normalized = normalize_path(path);
        retry_operation(|| fs::remove_dir_all(&normalized).map_err(TaskError::from)).await
    }

    async fn rename_file(&self, from: &Path, to: &Path) -> Result<(), TaskError> {
        let from_norm = normalize_path(from);
        let to_norm = normalize_path(to);
        retry_operation(|| fs::rename(&from_norm, &to_norm).map_err(TaskError::from)).await
    }

    async fn copy_file(&self, from: &Path, to: &Path) -> Result<(), TaskError> {
        let from_norm = normalize_path(from);
        let to_norm = normalize_path(to);
        retry_operation(|| {
            fs::copy(&from_norm, &to_norm).map_err(TaskError::from)?;
            Ok(())
        })
        .await
    }

    async fn file_metadata(&self, path: &Path) -> Result<fs::Metadata, TaskError> {
        let normalized = normalize_path(path);
        retry_operation(|| fs::metadata(&normalized).map_err(TaskError::from)).await
    }
}

// Helper function to recursively visit directories
fn visit_dirs(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), TaskError> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir).map_err(TaskError::from)? {
            let entry = entry.map_err(TaskError::from)?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, files)?;
            } else {
                files.push(path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tokio::test;

    fn test_dir() -> PathBuf {
        let mut dir = env::temp_dir();
        dir.push("fs_operations_test");
        dir
    }

    async fn setup() -> FileSystemOperationsImpl {
        let fs = FileSystemOperationsImpl::new();
        let test_dir = test_dir();
        let _ = fs::remove_dir_all(&test_dir);
        fs.create_dir(&test_dir).await.unwrap();
        fs
    }

    async fn cleanup() {
        let _ = fs::remove_dir_all(test_dir());
    }

    #[test]
    async fn test_file_operations() {
        let fs = setup().await;
        let test_dir = test_dir();

        // Test file creation and reading
        let file_path = test_dir.join("test.txt");
        let content = "Hello, World!";
        fs.write_file(&file_path, content).await.unwrap();
        assert!(fs.file_exists(&file_path).await);
        assert_eq!(fs.read_file(&file_path).await.unwrap(), content);

        // Test file copying
        let copy_path = test_dir.join("test_copy.txt");
        fs.copy_file(&file_path, &copy_path).await.unwrap();
        assert_eq!(fs.read_file(&copy_path).await.unwrap(), content);

        // Test file renaming
        let renamed_path = test_dir.join("test_renamed.txt");
        fs.rename_file(&copy_path, &renamed_path).await.unwrap();
        assert!(!fs.file_exists(&copy_path).await);
        assert!(fs.file_exists(&renamed_path).await);

        // Test metadata
        let metadata = fs.file_metadata(&file_path).await.unwrap();
        assert_eq!(metadata.len(), content.len() as u64);

        // Test directory operations
        let sub_dir = test_dir.join("subdir");
        fs.create_dir(&sub_dir).await.unwrap();
        assert!(sub_dir.exists());

        // Test recursive listing
        let nested_file = sub_dir.join("nested.txt");
        fs.write_file(&nested_file, "nested").await.unwrap();
        let files = fs.list_files(&test_dir).await.unwrap();
        assert!(files.contains(&file_path));
        assert!(files.contains(&renamed_path));
        assert!(files.contains(&nested_file));

        // Test cleanup
        fs.delete_file(&file_path).await.unwrap();
        fs.delete_file(&renamed_path).await.unwrap();
        fs.delete_dir(&test_dir).await.unwrap();
        assert!(!test_dir.exists());

        cleanup().await;
    }

    #[test]
    async fn test_path_normalization() {
        let path = Path::new("dir/./subdir/../file.txt");
        let normalized = normalize_path(path);
        assert_eq!(normalized, PathBuf::from("dir/file.txt"));
    }

    #[test]
    async fn test_error_handling() {
        let fs = FileSystemOperationsImpl::new();
        let not_found = Path::new("nonexistent.txt");
        match fs.read_file(not_found).await {
            Err(TaskError::IoError(e)) => assert_eq!(e.kind(), ErrorKind::NotFound),
            _ => panic!("Expected IoError"),
        }
    }
}
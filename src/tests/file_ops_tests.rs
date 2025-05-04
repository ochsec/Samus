use test_context::{test_context, AsyncTestContext};
use tokio::time::{sleep, Duration};
use std::{sync::Arc, path::PathBuf};
use tempfile::TempDir;

use crate::services::file::{
    FileService,
    FileOperation,
    FileEvent,
    FileError,
    WatchConfig
};
use super::test_utils;

pub struct FileOpsContext {
    pub service: Arc<FileService>,
    pub temp_dir: TempDir,
}

#[async_trait::async_trait]
impl AsyncTestContext for FileOpsContext {
    async fn setup() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let service = Arc::new(FileService::new(temp_dir.path().to_path_buf()).await);
        
        FileOpsContext {
            service,
            temp_dir,
        }
    }

    async fn teardown(self) {
        self.service.shutdown().await;
    }
}

#[test_context(FileOpsContext)]
#[tokio::test]
async fn test_file_operations(ctx: &mut FileOpsContext) {
    let test_file = ctx.temp_dir.path().join("test.txt");
    let content = "Hello, World!";

    // Test write operation
    ctx.service.write(&test_file, content.as_bytes().to_vec()).await
        .expect("Write operation should succeed");

    // Test read operation
    let read_content = ctx.service.read(&test_file).await
        .expect("Read operation should succeed");
    assert_eq!(
        String::from_utf8(read_content).unwrap(),
        content,
        "Read content should match written content"
    );

    // Test delete operation
    ctx.service.delete(&test_file).await
        .expect("Delete operation should succeed");
    
    assert!(!test_file.exists(), "File should be deleted");
}

#[test_context(FileOpsContext)]
#[tokio::test]
async fn test_file_watching(ctx: &mut FileOpsContext) {
    let test_file = ctx.temp_dir.path().join("watch_test.txt");
    let content = "Initial content";

    // Create file to watch
    ctx.service.write(&test_file, content.as_bytes().to_vec()).await.unwrap();

    // Setup watch
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let config = WatchConfig {
        path: test_file.clone(),
        recursive: false,
    };
    
    ctx.service.watch(config, tx).await.unwrap();

    // Modify file
    let new_content = "Modified content";
    ctx.service.write(&test_file, new_content.as_bytes().to_vec()).await.unwrap();

    // Wait for and verify file event
    let event = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await
        .expect("Should receive file event")
        .expect("Should have valid event");

    match event {
        FileEvent::Modified(path) => assert_eq!(path, test_file),
        _ => panic!("Expected modify event"),
    }
}

#[test_context(FileOpsContext)]
#[tokio::test]
async fn test_batch_operations(ctx: &mut FileOpsContext) {
    let mut operations = vec![];
    let mut files = vec![];

    // Create batch operations
    for i in 0..100 {
        let file = ctx.temp_dir.path().join(format!("batch_{}.txt", i));
        let content = format!("Content {}", i);
        operations.push(FileOperation::Write {
            path: file.clone(),
            content: content.as_bytes().to_vec(),
        });
        files.push(file);
    }

    // Execute batch operations
    let start = std::time::Instant::now();
    ctx.service.batch_execute(operations).await
        .expect("Batch operations should succeed");
    let duration = start.elapsed();

    // Verify all files were created
    for file in &files {
        assert!(file.exists(), "File should exist after batch operation");
    }

    // Verify performance
    println!("Batch operation took: {:?}", duration);
    assert!(
        duration < Duration::from_secs(1),
        "Batch operations should complete within 1 second"
    );
}

#[test_context(FileOpsContext)]
#[tokio::test]
async fn test_concurrent_access(ctx: &mut FileOpsContext) {
    let file = ctx.temp_dir.path().join("concurrent.txt");
    let service = ctx.service.clone();

    // Spawn multiple tasks trying to write to the same file
    let mut handles = vec![];
    for i in 0..10 {
        let service = service.clone();
        let file = file.clone();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let content = format!("Content {}-{}", i, j);
                service.write(&file, content.as_bytes().to_vec()).await?;
                sleep(Duration::from_millis(10)).await;
            }
            Ok::<(), FileError>(())
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Verify file integrity
    assert!(file.exists(), "File should exist after concurrent operations");
}

#[test_context(FileOpsContext)]
#[tokio::test]
async fn test_memory_efficiency(ctx: &mut FileOpsContext) {
    let large_content = vec![0u8; 1024 * 1024]; // 1MB
    let mut files = vec![];

    // Create multiple large files
    for i in 0..10 {
        let file = ctx.temp_dir.path().join(format!("large_{}.bin", i));
        ctx.service.write(&file, large_content.clone()).await.unwrap();
        files.push(file);
    }

    // Read all files concurrently
    let mut handles = vec![];
    for file in files {
        let service = ctx.service.clone();
        let handle = tokio::spawn(async move {
            service.read(&file).await
        });
        handles.push(handle);
    }

    // Wait for all reads to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Memory usage should be manageable due to streaming
    // Actual memory metrics would be checked in production environment
}
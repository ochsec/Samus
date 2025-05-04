use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use tokio::time::Duration;
use tempfile::TempDir;
use std::{sync::Arc, path::PathBuf};

use crate::services::file::{FileService, FileOperation};
use super::bench_utils;

fn benchmark_file_operations(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();

    let mut group = c.benchmark_group("file_operations");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("write_read_small", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let service = FileService::new(temp_dir.path().to_path_buf()).await;
            let test_file = temp_dir.path().join("test.txt");
            let content = "Hello, World!".as_bytes().to_vec();
            
            service.write(&test_file, content.clone()).await.unwrap();
            let _ = service.read(&test_file).await.unwrap();
            
            service.shutdown().await;
        });
    });

    let large_content = vec![0u8; 1024 * 1024]; // 1MB
    group.bench_function("write_read_large", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let service = FileService::new(temp_dir.path().to_path_buf()).await;
            let test_file = temp_dir.path().join("test_large.bin");
            
            service.write(&test_file, large_content.clone()).await.unwrap();
            let _ = service.read(&test_file).await.unwrap();
            
            service.shutdown().await;
        });
    });

    group.finish();
}

fn benchmark_batch_operations(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();
    
    let mut group = c.benchmark_group("batch_operations");
    group.measurement_time(Duration::from_secs(15));

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("batch_files", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let service = FileService::new(temp_dir.path().to_path_buf()).await;
                let content = "Test content".as_bytes().to_vec();
                
                let mut operations = Vec::with_capacity(size);
                for i in 0..size {
                    let file = temp_dir.path().join(format!("test_{}.txt", i));
                    operations.push(FileOperation::Write {
                        path: file,
                        content: content.clone(),
                    });
                }
                
                service.batch_execute(operations).await.unwrap();
                service.shutdown().await;
            });
        });
    }

    group.finish();
}

fn benchmark_parallel_operations(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();

    let mut group = c.benchmark_group("parallel_operations");
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("concurrent_access", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let service = Arc::new(FileService::new(temp_dir.path().to_path_buf()).await);
            let mut handles = vec![];
            
            for i in 0..10 {
                let service = service.clone();
                let dir = temp_dir.path().to_path_buf();
                let handle = tokio::spawn(async move {
                    let file = dir.join(format!("concurrent_{}.txt", i));
                    let content = format!("Content {}", i).as_bytes().to_vec();
                    
                    for _ in 0..100 {
                        service.write(&file, content.clone()).await.unwrap();
                        let _ = service.read(&file).await.unwrap();
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.await.unwrap();
            }
            
            service.shutdown().await;
        });
    });

    group.finish();
}

fn benchmark_streaming_operations(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();

    let mut group = c.benchmark_group("streaming_operations");
    group.measurement_time(Duration::from_secs(20));

    let large_content = vec![0u8; 10 * 1024 * 1024]; // 10MB
    group.bench_function("stream_large_files", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let service = FileService::new(temp_dir.path().to_path_buf()).await;
            let mut files = Vec::new();
            
            // Create multiple large files
            for i in 0..5 {
                let file = temp_dir.path().join(format!("large_{}.bin", i));
                service.write(&file, large_content.clone()).await.unwrap();
                files.push(file);
            }
            
            // Read all files concurrently
            let mut handles = Vec::new();
            for file in files {
                let service = service.clone();
                let handle = tokio::spawn(async move {
                    let _ = service.read(&file).await.unwrap();
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.await.unwrap();
            }
            
            service.shutdown().await;
        });
    });

    group.finish();
}

criterion_group!(
    name = file_ops_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(30));
    targets = benchmark_file_operations,
             benchmark_batch_operations,
             benchmark_parallel_operations,
             benchmark_streaming_operations
);

criterion_main!(file_ops_benches);
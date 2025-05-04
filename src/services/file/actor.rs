use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use futures::Stream;
use crate::actor::{Actor, ActorError, ActorPath};
use crate::services::file::buffer::BufferPool;
use crate::services::file::cache::{FileCache, CacheConfig};
use super::FileEvent;

#[derive(Debug)]
pub enum FileCommand {
    Read { path: PathBuf },
    Write { path: PathBuf, contents: Vec<u8> },
    Watch { path: PathBuf },
    Batch { operations: Vec<FileOperation> },
}

#[derive(Debug)]
pub enum FileOperation {
    Read(PathBuf),
    Write(PathBuf, Vec<u8>),
}

#[derive(Debug)]
pub struct FileResponse {
    pub result: Result<Vec<u8>, std::io::Error>,
    pub path: PathBuf,
}

pub struct FileActor {
    buffer_pool: Arc<BufferPool>,
    cache: Arc<FileCache>,
    metrics: Arc<crate::actor::MetricsCollector>,
    event_tx: mpsc::Sender<FileEvent>,
}

impl FileActor {
    pub fn new(
        buffer_pool: Arc<BufferPool>,
        cache_config: CacheConfig,
        metrics: Arc<crate::actor::MetricsCollector>,
        event_tx: mpsc::Sender<FileEvent>,
    ) -> Self {
        Self {
            buffer_pool,
            cache: Arc::new(FileCache::new(cache_config)),
            metrics,
            event_tx,
        }
    }

    async fn handle_read(&self, path: PathBuf) -> Result<Vec<u8>, std::io::Error> {
        // Check cache first
        if let Some(entry) = self.cache.get(&path) {
            self.metrics.increment_counter("file_cache_hits");
            return Ok(entry.data.to_vec());
        }

        self.metrics.increment_counter("file_cache_misses");
        
        // Get appropriate buffer from pool
        let metadata = tokio::fs::metadata(&path).await?;
        let mut buffer = self.buffer_pool.acquire(metadata.len() as usize);

        // Read file
        let mut file = tokio::fs::File::open(&path).await?;
        use tokio::io::AsyncReadExt;
        let n = file.read(buffer.as_mut_slice()).await?;
        buffer.resize(n);

        // Cache result
        let data = buffer.as_slice().to_vec();
        self.cache.insert(path, data.clone());

        Ok(data)
    }

    async fn handle_write(&self, path: PathBuf, contents: Vec<u8>) -> Result<(), std::io::Error> {
        // Get buffer from pool
        let mut buffer = self.buffer_pool.acquire(contents.len());
        buffer.as_mut_slice()[..contents.len()].copy_from_slice(&contents);

        // Write file
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(&path).await?;
        file.write_all(buffer.as_slice()).await?;
        file.sync_all().await?;

        // Invalidate cache
        self.cache.invalidate(&path);

        // Notify watchers
        if let Err(e) = self.event_tx.send(FileEvent::Modified { path: path.clone() }).await {
            eprintln!("Failed to send file event: {}", e);
        }

        Ok(())
    }

    async fn handle_batch(&self, operations: Vec<FileOperation>) -> Result<Vec<FileResponse>, std::io::Error> {
        let mut responses = Vec::with_capacity(operations.len());

        for op in operations {
            match op {
                FileOperation::Read(path) => {
                    let result = self.handle_read(path.clone()).await;
                    responses.push(FileResponse { path, result });
                }
                FileOperation::Write(path, contents) => {
                    let result = self.handle_write(path.clone(), contents).await
                        .map(|_| Vec::new());
                    responses.push(FileResponse { path, result });
                }
            }
        }

        Ok(responses)
    }
}

#[async_trait::async_trait]
impl Actor for FileActor {
    type Message = FileCommand;

    async fn handle(&mut self, msg: Self::Message) -> Result<(), ActorError> {
        let start = std::time::Instant::now();
        
        let result = match msg {
            FileCommand::Read { path } => {
                self.handle_read(path).await.map(|_| ())
            }
            FileCommand::Write { path, contents } => {
                self.handle_write(path, contents).await
            }
            FileCommand::Watch { path } => {
                if let Err(e) = self.event_tx.send(FileEvent::Created { path }).await {
                    eprintln!("Failed to send watch event: {}", e);
                }
                Ok(())
            }
            FileCommand::Batch { operations } => {
                self.handle_batch(operations).await.map(|_| ())
            }
        };

        let duration = start.elapsed();
        self.metrics.record_duration("file_operation", duration);

        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                self.metrics.increment_counter("file_errors");
                Err(ActorError::Internal(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_file_actor() {
        let buffer_pool = BufferPool::new(10, 5, 2);
        let cache_config = CacheConfig::default();
        let metrics = Arc::new(crate::actor::MetricsCollector::new());
        let (event_tx, mut event_rx) = mpsc::channel(100);

        let mut actor = FileActor::new(buffer_pool, cache_config, metrics, event_tx);

        // Test write
        let test_path = PathBuf::from("test.txt");
        let contents = b"test content".to_vec();
        let write_cmd = FileCommand::Write {
            path: test_path.clone(),
            contents: contents.clone(),
        };
        actor.handle(write_cmd).await.unwrap();

        // Verify write event
        if let Some(FileEvent::Modified { path }) = event_rx.recv().await {
            assert_eq!(path, test_path);
        }

        // Test read
        let read_cmd = FileCommand::Read {
            path: test_path.clone(),
        };
        actor.handle(read_cmd).await.unwrap();

        // Clean up
        tokio::fs::remove_file(test_path).await.unwrap();
    }
}
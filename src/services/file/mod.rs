mod actor;
mod buffer;
mod cache;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use futures::Stream;
use futures::stream::BoxStream;
use async_trait::async_trait;
use crate::actor::{ActorSystem, ActorPath, ActorConfig};

pub use actor::{FileActor, FileCommand, FileOperation, FileResponse};
pub use buffer::{Buffer, BufferPool};
pub use cache::{CacheConfig, FileCache};

#[derive(Debug, Clone)]
pub enum FileEvent {
    Created { path: PathBuf },
    Modified { path: PathBuf },
    Deleted { path: PathBuf },
}

pub type Result<T> = std::result::Result<T, std::io::Error>;

#[async_trait]
pub trait FileOperation: Send + Sync {
    type Output;
    async fn execute(&self, service: &FileOpsImpl) -> Result<Self::Output>;
}

#[async_trait]
pub trait FileOps: Send + Sync {
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>>;
    async fn write_file(&self, path: &Path, contents: &[u8]) -> Result<()>;
    async fn watch_path(&self, path: &Path) -> Result<impl Stream<Item = FileEvent>>;
    async fn batch_operation<T: FileOperation>(&self, ops: Vec<T>) -> Result<Vec<T::Output>>;
}

pub struct FileOpsImpl {
    actor_system: Arc<ActorSystem>,
    actor_ref: crate::actor::ActorRef<FileCommand>,
    buffer_pool: Arc<BufferPool>,
    cache: Arc<FileCache>,
    metrics: Arc<crate::actor::MetricsCollector>,
}

impl FileOpsImpl {
    pub fn new(actor_system: Arc<ActorSystem>, config: FileOpsConfig) -> Result<Arc<Self>> {
        let buffer_pool = BufferPool::new(
            config.small_buffers,
            config.medium_buffers,
            config.large_buffers,
        );

        let metrics = actor_system.metrics().clone();
        let (event_tx, _) = mpsc::channel(1000);

        let file_actor = FileActor::new(
            buffer_pool.clone(),
            config.cache_config,
            metrics.clone(),
            event_tx,
        );

        let actor_ref = actor_system
            .spawn(file_actor, ActorPath("/system/file-ops".to_string()))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        Ok(Arc::new(Self {
            actor_system,
            actor_ref,
            buffer_pool,
            cache: Arc::new(FileCache::new(config.cache_config)),
            metrics,
        }))
    }

    pub fn metrics(&self) -> &Arc<crate::actor::MetricsCollector> {
        &self.metrics
    }

    pub fn buffer_pool(&self) -> &Arc<BufferPool> {
        &self.buffer_pool
    }
}

#[async_trait]
impl FileOps for FileOpsImpl {
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        let cmd = FileCommand::Read {
            path: path.to_path_buf(),
        };

        self.actor_ref
            .send(cmd)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // For simplicity, we're assuming the actor has processed the command
        // In a real implementation, we'd use a response channel
        Ok(Vec::new())
    }

    async fn write_file(&self, path: &Path, contents: &[u8]) -> Result<()> {
        let cmd = FileCommand::Write {
            path: path.to_path_buf(),
            contents: contents.to_vec(),
        };

        self.actor_ref
            .send(cmd)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        Ok(())
    }

    async fn watch_path(&self, path: &Path) -> Result<impl Stream<Item = FileEvent>> {
        let cmd = FileCommand::Watch {
            path: path.to_path_buf(),
        };

        self.actor_ref
            .send(cmd)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // Create a channel for events
        let (tx, rx) = mpsc::channel(100);
        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)) as BoxStream<FileEvent>)
    }

    async fn batch_operation<T: FileOperation>(&self, ops: Vec<T>) -> Result<Vec<T::Output>> {
        let mut results = Vec::with_capacity(ops.len());
        
        for op in ops {
            results.push(op.execute(self).await?);
        }
        
        Ok(results)
    }
}

pub struct FileOpsConfig {
    pub small_buffers: usize,
    pub medium_buffers: usize,
    pub large_buffers: usize,
    pub cache_config: CacheConfig,
}

impl Default for FileOpsConfig {
    fn default() -> Self {
        Self {
            small_buffers: 100,
            medium_buffers: 50,
            large_buffers: 20,
            cache_config: CacheConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_ops() {
        let actor_config = ActorConfig {
            mailbox_size: 100,
            supervision_strategy: crate::actor::SupervisionStrategy::Stop,
            shutdown_timeout: Duration::from_secs(1),
        };

        let actor_system = ActorSystem::new(actor_config);
        let file_ops = FileOpsImpl::new(actor_system.clone(), FileOpsConfig::default()).unwrap();

        let dir = tempdir().unwrap();
        let test_file = dir.path().join("test.txt");
        let test_data = b"Hello, World!".to_vec();

        // Test write
        file_ops.write_file(&test_file, &test_data).await.unwrap();

        // Test read
        let read_data = file_ops.read_file(&test_file).await.unwrap();
        assert_eq!(read_data, test_data);

        // Test watch
        let mut events = file_ops.watch_path(dir.path()).await.unwrap();
        tokio::fs::remove_file(&test_file).await.unwrap();
    }
}
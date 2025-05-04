use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Semaphore;

use super::command::Event;
use super::snapshot::{Snapshot, SnapshotStrategy, HybridSnapshotStrategy, SnapshotError};

const MAX_CONCURRENT_OPERATIONS: usize = 32;
const DEFAULT_BATCH_SIZE: usize = 100;

#[derive(Debug)]
pub enum EventStoreError {
    ConcurrencyError(String),
    SerializationError(String),
    StorageError(String),
    SnapshotError(SnapshotError),
}

impl std::fmt::Display for EventStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EventStoreError::ConcurrencyError(msg) => write!(f, "Concurrency error: {}", msg),
            EventStoreError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            EventStoreError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            EventStoreError::SnapshotError(e) => write!(f, "Snapshot error: {}", e),
        }
    }
}

impl Error for EventStoreError {}

// Event metadata for versioning and tracking
#[derive(Clone)]
pub struct EventMetadata {
    pub version: u32,
    pub schema_version: u32,
    pub timestamp: u64,
}

// Core EventStore trait
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append_events(
        &self,
        stream_id: &str,
        events: Vec<Box<dyn Event>>,
        expected_version: i64,
    ) -> Result<(), EventStoreError>;

    async fn read_events(
        &self,
        stream_id: &str,
        start: i64,
        count: i64,
    ) -> Result<Vec<Box<dyn Event>>, EventStoreError>;

    async fn read_snapshot(
        &self,
        stream_id: &str,
    ) -> Result<Option<Snapshot>, EventStoreError>;

    async fn create_snapshot(
        &self,
        stream_id: &str,
        snapshot: Snapshot,
    ) -> Result<(), EventStoreError>;
}

// Optimized in-memory event store implementation
pub struct InMemoryEventStore {
    events: Arc<RwLock<HashMap<String, Vec<(Box<dyn Event>, EventMetadata)>>>>,
    snapshots: Arc<RwLock<HashMap<String, Snapshot>>>,
    snapshot_strategy: Box<dyn SnapshotStrategy>,
    semaphore: Arc<Semaphore>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        // Use hybrid snapshot strategy by default
        let snapshot_strategy = Box::new(HybridSnapshotStrategy::new(100, 3600));
        
        InMemoryEventStore {
            events: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            snapshot_strategy,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_OPERATIONS)),
        }
    }

    async fn process_events_batch(
        &self,
        events: Vec<Box<dyn Event>>,
    ) -> Result<Vec<(Box<dyn Event>, EventMetadata)>, EventStoreError> {
        // Process events in parallel with controlled concurrency
        let results = stream::iter(events)
            .map(|event| {
                let permit = self.semaphore.clone().acquire_owned();
                async move {
                    let _permit = permit.await;
                    
                    let metadata = EventMetadata {
                        version: event.version(),
                        schema_version: event.schema_version(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    
                    Ok((event, metadata))
                }
            })
            .buffer_unordered(MAX_CONCURRENT_OPERATIONS)
            .collect::<Vec<_>>()
            .await;

        // Aggregate results
        let mut processed_events = Vec::new();
        for result in results {
            processed_events.push(result?);
        }
        
        Ok(processed_events)
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn append_events(
        &self,
        stream_id: &str,
        events: Vec<Box<dyn Event>>,
        expected_version: i64,
    ) -> Result<(), EventStoreError> {
        let processed_events = self.process_events_batch(events).await?;
        
        let mut events_lock = self.events.write();
        let stream_events = events_lock.entry(stream_id.to_string())
            .or_insert_with(Vec::new);

        // Optimistic concurrency check
        let current_version = stream_events.len() as i64 - 1;
        if expected_version >= 0 && current_version != expected_version {
            return Err(EventStoreError::ConcurrencyError(
                format!("Expected version {}, but current version is {}", 
                    expected_version, current_version)
            ));
        }

        // Append events in batches
        for chunk in processed_events.chunks(DEFAULT_BATCH_SIZE) {
            stream_events.extend(chunk.to_vec());
        }

        // Check if snapshot should be created
        if self.snapshot_strategy.should_snapshot(stream_events.len() as u32) {
            // Create snapshot asynchronously
            let stream_id = stream_id.to_string();
            let events = stream_events.clone();
            let snapshots = self.snapshots.clone();
            
            tokio::spawn(async move {
                let snapshot_data = events.last()
                    .map(|(event, _)| event.serialize().await.ok())
                    .flatten()
                    .unwrap_or_default();

                let snapshot = Snapshot::new(
                    stream_id.clone(),
                    events.len() as u32,
                    snapshot_data,
                );

                snapshots.write().insert(stream_id, snapshot);
            });
        }

        Ok(())
    }

    async fn read_events(
        &self,
        stream_id: &str,
        start: i64,
        count: i64,
    ) -> Result<Vec<Box<dyn Event>>, EventStoreError> {
        let events_lock = self.events.read();
        
        let stream_events = events_lock.get(stream_id)
            .ok_or_else(|| EventStoreError::StorageError(
                format!("Stream {} not found", stream_id)
            ))?;

        let start_idx = start.max(0) as usize;
        let end_idx = (start + count).min(stream_events.len() as i64) as usize;

        Ok(stream_events[start_idx..end_idx]
            .iter()
            .map(|(event, _)| event.clone())
            .collect())
    }

    async fn read_snapshot(
        &self,
        stream_id: &str,
    ) -> Result<Option<Snapshot>, EventStoreError> {
        Ok(self.snapshots.read().get(stream_id).cloned())
    }

    async fn create_snapshot(
        &self,
        stream_id: &str,
        snapshot: Snapshot,
    ) -> Result<(), EventStoreError> {
        self.snapshots.write().insert(stream_id.to_string(), snapshot);
        Ok(())
    }
}

// Metrics for monitoring event store performance
pub struct EventStoreMetrics {
    pub events_processed: u64,
    pub snapshots_created: u64,
    pub average_event_processing_time_ms: f64,
    pub average_batch_size: f64,
}

impl EventStoreMetrics {
    pub fn new() -> Self {
        EventStoreMetrics {
            events_processed: 0,
            snapshots_created: 0,
            average_event_processing_time_ms: 0.0,
            average_batch_size: 0.0,
        }
    }
}
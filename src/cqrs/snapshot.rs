use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub aggregate_id: String,
    pub version: u32,
    pub timestamp: u64,
    pub data: Vec<u8>,
}

impl Snapshot {
    pub fn new(aggregate_id: String, version: u32, data: Vec<u8>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Snapshot {
            aggregate_id,
            version,
            timestamp,
            data,
        }
    }

    pub fn is_stale(&self, max_age_secs: u64) -> bool {
        let current = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        current - self.timestamp > max_age_secs
    }
}

// Snapshot strategy for determining when to create snapshots
pub trait SnapshotStrategy: Send + Sync {
    fn should_snapshot(&self, events_since_snapshot: u32) -> bool;
}

// Event count based snapshot strategy
pub struct EventCountSnapshotStrategy {
    threshold: u32,
}

impl EventCountSnapshotStrategy {
    pub fn new(threshold: u32) -> Self {
        EventCountSnapshotStrategy { threshold }
    }
}

impl SnapshotStrategy for EventCountSnapshotStrategy {
    fn should_snapshot(&self, events_since_snapshot: u32) -> bool {
        events_since_snapshot >= self.threshold
    }
}

// Time based snapshot strategy
pub struct TimeBasedSnapshotStrategy {
    interval_secs: u64,
    last_snapshot: SystemTime,
}

impl TimeBasedSnapshotStrategy {
    pub fn new(interval_secs: u64) -> Self {
        TimeBasedSnapshotStrategy {
            interval_secs,
            last_snapshot: SystemTime::now(),
        }
    }
}

impl SnapshotStrategy for TimeBasedSnapshotStrategy {
    fn should_snapshot(&self, _events_since_snapshot: u32) -> bool {
        let elapsed = SystemTime::now()
            .duration_since(self.last_snapshot)
            .unwrap_or_default()
            .as_secs();
        
        elapsed >= self.interval_secs
    }
}

// Hybrid snapshot strategy combining event count and time-based approaches
pub struct HybridSnapshotStrategy {
    event_strategy: EventCountSnapshotStrategy,
    time_strategy: TimeBasedSnapshotStrategy,
}

impl HybridSnapshotStrategy {
    pub fn new(event_threshold: u32, time_interval_secs: u64) -> Self {
        HybridSnapshotStrategy {
            event_strategy: EventCountSnapshotStrategy::new(event_threshold),
            time_strategy: TimeBasedSnapshotStrategy::new(time_interval_secs),
        }
    }
}

impl SnapshotStrategy for HybridSnapshotStrategy {
    fn should_snapshot(&self, events_since_snapshot: u32) -> bool {
        self.event_strategy.should_snapshot(events_since_snapshot) ||
        self.time_strategy.should_snapshot(events_since_snapshot)
    }
}

// Snapshot storage error type
#[derive(Debug)]
pub enum SnapshotError {
    SerializationError(String),
    StorageError(String),
    NotFound,
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SnapshotError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            SnapshotError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            SnapshotError::NotFound => write!(f, "Snapshot not found"),
        }
    }
}

impl Error for SnapshotError {}
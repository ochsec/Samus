use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

// Aggregate trait defines entity that can be built from events
pub trait Aggregate: Send + Sync {
    type Error: Error;
    
    fn apply_event(&mut self, event: Event) -> Result<(), Self::Error>;
    fn current_version(&self) -> u32;
}

// Base Command trait
pub trait Command: Send + Sync {
    type Aggregate: Aggregate;
    type Error: Error;
}

// Validation error type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub message: String,
    pub field: Option<String>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.field {
            Some(field) => write!(f, "Validation error on field '{}': {}", field, self.message),
            None => write!(f, "Validation error: {}", self.message),
        }
    }
}

impl Error for ValidationError {}

// Event trait with versioning support
#[async_trait]
pub trait Event: Send + Sync + Clone {
    fn version(&self) -> u32;
    fn aggregate_id(&self) -> &str;
    fn event_type(&self) -> &str;
    
    // Event schema version for compatibility
    fn schema_version(&self) -> u32;
    
    // Serialization helpers
    async fn serialize(&self) -> Result<Vec<u8>, Box<dyn Error>>;
    async fn deserialize(bytes: &[u8]) -> Result<Self, Box<dyn Error>> where Self: Sized;
}

// Command handler trait
#[async_trait]
pub trait CommandHandler<C: Command>: Send + Sync {
    async fn handle(&self, cmd: C) -> Result<Vec<Box<dyn Event>>, C::Error>;
    
    fn validate(&self, cmd: &C) -> Result<(), ValidationError>;
    
    async fn persist_events(&self, events: Vec<Box<dyn Event>>) -> Result<(), Box<dyn Error>>;
}

// Memory pool for event instances
pub struct EventPool<T: Event> {
    pool: Vec<T>,
}

impl<T: Event> EventPool<T> {
    pub fn new(capacity: usize) -> Self {
        EventPool {
            pool: Vec::with_capacity(capacity),
        }
    }

    pub fn acquire(&mut self) -> Option<T> {
        self.pool.pop()
    }

    pub fn release(&mut self, event: T) {
        self.pool.push(event);
    }
}
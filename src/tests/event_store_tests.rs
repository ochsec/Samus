use test_context::{test_context, AsyncTestContext};
use tokio::time::{sleep, Duration};
use std::sync::Arc;
use uuid::Uuid;

use crate::cqrs::{
    EventStore,
    Command,
    Event,
    Snapshot,
    Aggregate,
    EventMetadata,
    AggregateError
};
use super::test_utils;

// Test context for event store tests
pub struct EventStoreContext {
    pub store: EventStore,
}

#[async_trait::async_trait]
impl AsyncTestContext for EventStoreContext {
    async fn setup() -> Self {
        let store = EventStore::new().await;
        EventStoreContext { store }
    }

    async fn teardown(self) {
        self.store.shutdown().await;
    }
}

// Test aggregate implementation
#[derive(Debug, Clone)]
struct TestAggregate {
    id: Uuid,
    value: i32,
    version: u64,
}

impl TestAggregate {
    fn new(id: Uuid) -> Self {
        Self {
            id,
            value: 0,
            version: 0,
        }
    }
}

#[async_trait::async_trait]
impl Aggregate for TestAggregate {
    fn id(&self) -> Uuid {
        self.id
    }

    fn version(&self) -> u64 {
        self.version
    }

    async fn apply_event(&mut self, event: Event) -> Result<(), AggregateError> {
        match event {
            Event::Custom(data) => {
                if let Ok(value) = data.parse::<i32>() {
                    self.value += value;
                    self.version += 1;
                    Ok(())
                } else {
                    Err(AggregateError::InvalidEvent)
                }
            }
            _ => Ok(()),
        }
    }
}

// Test commands
#[derive(Debug)]
enum TestCommand {
    Increment(i32),
    Decrement(i32),
}

impl Command for TestCommand {
    fn aggregate_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}

#[test_context(EventStoreContext)]
#[tokio::test]
async fn test_event_sourcing(ctx: &mut EventStoreContext) {
    let aggregate_id = Uuid::new_v4();
    let mut aggregate = TestAggregate::new(aggregate_id);

    // Store some events
    for i in 1..=5 {
        let event = Event::Custom(i.to_string());
        let metadata = EventMetadata::new(aggregate_id);
        
        ctx.store.append_event(event, metadata).await.unwrap();
    }

    // Replay events
    let events = ctx.store.get_events(aggregate_id).await.unwrap();
    for event in events {
        aggregate.apply_event(event).await.unwrap();
    }

    assert_eq!(aggregate.value, 15, "Event replay should yield correct state");
    assert_eq!(aggregate.version, 5, "Version should match event count");
}

#[test_context(EventStoreContext)]
#[tokio::test]
async fn test_snapshots(ctx: &mut EventStoreContext) {
    let aggregate_id = Uuid::new_v4();
    let mut aggregate = TestAggregate::new(aggregate_id);

    // Create initial state through events
    for i in 1..=10 {
        let event = Event::Custom(i.to_string());
        let metadata = EventMetadata::new(aggregate_id);
        ctx.store.append_event(event, metadata).await.unwrap();
        aggregate.apply_event(Event::Custom(i.to_string())).await.unwrap();
    }

    // Create snapshot
    let snapshot = Snapshot::new(aggregate_id, aggregate.clone(), 10);
    ctx.store.save_snapshot(snapshot).await.unwrap();

    // Verify snapshot restoration
    let restored = ctx.store.get_latest_snapshot(aggregate_id).await.unwrap();
    let restored_aggregate = restored.state::<TestAggregate>().unwrap();

    assert_eq!(restored_aggregate.value, aggregate.value, "Snapshot should preserve state");
    assert_eq!(restored_aggregate.version, aggregate.version, "Snapshot should preserve version");
}

#[test_context(EventStoreContext)]
#[tokio::test]
async fn test_concurrent_events(ctx: &mut EventStoreContext) {
    let aggregate_id = Uuid::new_v4();
    
    // Simulate concurrent event appends
    let mut handles = vec![];
    for i in 0..100 {
        let store = ctx.store.clone();
        let handle = tokio::spawn(async move {
            let event = Event::Custom(i.to_string());
            let metadata = EventMetadata::new(aggregate_id);
            store.append_event(event, metadata).await
        });
        handles.push(handle);
    }

    // Wait for all events to be processed
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Verify all events were stored
    let events = ctx.store.get_events(aggregate_id).await.unwrap();
    assert_eq!(events.len(), 100, "All concurrent events should be stored");
}

#[test_context(EventStoreContext)]
#[tokio::test]
async fn test_event_processing_performance(ctx: &mut EventStoreContext) {
    let aggregate_id = Uuid::new_v4();
    let start = std::time::Instant::now();
    
    // Process multiple events
    for i in 0..1000 {
        let event = Event::Custom(i.to_string());
        let metadata = EventMetadata::new(aggregate_id);
        ctx.store.append_event(event, metadata).await.unwrap();
    }

    let duration = start.elapsed();
    let events_per_second = 1000.0 / duration.as_secs_f64();
    
    assert!(duration.as_secs_f64() < 1.0, "Batch event processing should complete under 1 second");
    assert!(events_per_second > 1000.0, "Should process over 1000 events per second");
}
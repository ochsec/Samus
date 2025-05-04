use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use tokio::time::Duration;
use uuid::Uuid;
use std::sync::Arc;

use crate::cqrs::{EventStore, Event, EventMetadata, Snapshot, Aggregate};
use super::bench_utils;

#[derive(Debug, Clone)]
struct BenchAggregate {
    id: Uuid,
    value: i32,
    version: u64,
}

impl BenchAggregate {
    fn new(id: Uuid) -> Self {
        Self {
            id,
            value: 0,
            version: 0,
        }
    }
}

#[async_trait::async_trait]
impl Aggregate for BenchAggregate {
    fn id(&self) -> Uuid {
        self.id
    }

    fn version(&self) -> u64 {
        self.version
    }

    async fn apply_event(&mut self, event: Event) -> Result<(), Box<dyn std::error::Error>> {
        match event {
            Event::Custom(data) => {
                if let Ok(value) = data.parse::<i32>() {
                    self.value += value;
                    self.version += 1;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

fn benchmark_event_processing(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();

    let mut group = c.benchmark_group("event_processing");
    group.measurement_time(Duration::from_secs(10));

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("append_events", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let store = EventStore::new().await;
                let aggregate_id = Uuid::new_v4();
                
                for i in 0..size {
                    let event = Event::Custom(i.to_string());
                    let metadata = EventMetadata::new(aggregate_id);
                    store.append_event(event, metadata).await.unwrap();
                }
                
                store.shutdown().await;
            });
        });
    }

    group.finish();
}

fn benchmark_snapshot_operations(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();

    let mut group = c.benchmark_group("snapshot_operations");
    group.measurement_time(Duration::from_secs(10));

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("create_snapshots", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let store = EventStore::new().await;
                let aggregate_id = Uuid::new_v4();
                let mut aggregate = BenchAggregate::new(aggregate_id);
                
                // Create events
                for i in 0..size {
                    let event = Event::Custom(i.to_string());
                    let metadata = EventMetadata::new(aggregate_id);
                    store.append_event(event.clone(), metadata).await.unwrap();
                    aggregate.apply_event(event).await.unwrap();
                }
                
                // Create snapshot
                let snapshot = Snapshot::new(aggregate_id, aggregate.clone(), size as u64);
                store.save_snapshot(snapshot).await.unwrap();
                
                store.shutdown().await;
            });
        });
    }

    group.finish();
}

fn benchmark_event_replay(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();

    let mut group = c.benchmark_group("event_replay");
    group.measurement_time(Duration::from_secs(15));

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("replay_events", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let store = EventStore::new().await;
                let aggregate_id = Uuid::new_v4();
                
                // Create events
                for i in 0..size {
                    let event = Event::Custom(i.to_string());
                    let metadata = EventMetadata::new(aggregate_id);
                    store.append_event(event, metadata).await.unwrap();
                }
                
                // Replay events
                let mut aggregate = BenchAggregate::new(aggregate_id);
                let events = store.get_events(aggregate_id).await.unwrap();
                for event in events {
                    aggregate.apply_event(event).await.unwrap();
                }
                
                store.shutdown().await;
            });
        });
    }

    group.finish();
}

fn benchmark_concurrent_processing(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();

    let mut group = c.benchmark_group("concurrent_event_processing");
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("concurrent_aggregates", |b| {
        b.to_async(&rt).iter(|| async {
            let store = EventStore::new().await;
            let mut handles = vec![];
            
            // Process events for multiple aggregates concurrently
            for _ in 0..10 {
                let store = store.clone();
                let handle = tokio::spawn(async move {
                    let aggregate_id = Uuid::new_v4();
                    for i in 0..1000 {
                        let event = Event::Custom(i.to_string());
                        let metadata = EventMetadata::new(aggregate_id);
                        store.append_event(event, metadata).await.unwrap();
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.await.unwrap();
            }
            
            store.shutdown().await;
        });
    });

    group.finish();
}

criterion_group!(
    name = event_store_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(30));
    targets = benchmark_event_processing,
             benchmark_snapshot_operations,
             benchmark_event_replay,
             benchmark_concurrent_processing
);

criterion_main!(event_store_benches);
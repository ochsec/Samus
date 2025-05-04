use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use tokio::time::Duration;

use crate::actor::{ActorSystem, Message, Actor};
use super::bench_utils;

#[derive(Debug)]
struct BenchActor {
    count: u32,
}

impl BenchActor {
    fn new() -> Self {
        Self { count: 0 }
    }
}

#[async_trait::async_trait]
impl Actor for BenchActor {
    async fn handle_message(&mut self, msg: Message) -> Result<(), Box<dyn std::error::Error>> {
        match msg {
            Message::Text(_) => {
                self.count += 1;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

fn benchmark_message_latency(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();
    let metrics = bench_utils::create_metrics();

    let mut group = c.benchmark_group("actor_message_latency");
    group.throughput(Throughput::Elements(1u64));
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("single_message", |b| {
        b.to_async(&rt).iter(|| async {
            let system = ActorSystem::new(metrics.clone()).await;
            let actor = system.spawn_actor(BenchActor::new()).await.unwrap();
            
            actor.send(Message::Text("test".into())).await.unwrap();
            system.shutdown().await;
        });
    });

    group.finish();
}

fn benchmark_message_throughput(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();
    let metrics = bench_utils::create_metrics();

    let mut group = c.benchmark_group("actor_message_throughput");
    
    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("batch_messages", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let system = ActorSystem::new(metrics.clone()).await;
                let actor = system.spawn_actor(BenchActor::new()).await.unwrap();
                
                for _ in 0..size {
                    actor.send(Message::Text("test".into())).await.unwrap();
                }
                
                system.shutdown().await;
            });
        });
    }

    group.finish();
}

fn benchmark_concurrent_actors(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();
    let metrics = bench_utils::create_metrics();

    let mut group = c.benchmark_group("concurrent_actors");
    
    for actor_count in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*actor_count as u64));
        group.bench_with_input(BenchmarkId::new("spawn_actors", actor_count), actor_count, |b, &count| {
            b.to_async(&rt).iter(|| async {
                let system = ActorSystem::new(metrics.clone()).await;
                let mut actors = Vec::with_capacity(count);
                
                for _ in 0..count {
                    let actor = system.spawn_actor(BenchActor::new()).await.unwrap();
                    actors.push(actor);
                }
                
                for actor in &actors {
                    actor.send(Message::Text("test".into())).await.unwrap();
                }
                
                system.shutdown().await;
            });
        });
    }

    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let rt = bench_utils::setup_runtime();
    let metrics = bench_utils::create_metrics();

    let mut group = c.benchmark_group("actor_memory_usage");
    group.measurement_time(Duration::from_secs(20));
    
    group.bench_function("memory_efficiency", |b| {
        b.to_async(&rt).iter(|| async {
            let system = ActorSystem::new(metrics.clone()).await;
            let mut actors = Vec::with_capacity(1000);
            
            // Create 1000 actors
            for _ in 0..1000 {
                let actor = system.spawn_actor(BenchActor::new()).await.unwrap();
                actors.push(actor);
            }
            
            // Send 1000 messages to each actor
            for actor in &actors {
                for _ in 0..1000 {
                    actor.send(Message::Text("test".into())).await.unwrap();
                }
            }
            
            system.shutdown().await;
        });
    });

    group.finish();
}

criterion_group!(
    name = actor_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(30));
    targets = benchmark_message_latency,
             benchmark_message_throughput,
             benchmark_concurrent_actors,
             benchmark_memory_usage
);

criterion_main!(actor_benches);
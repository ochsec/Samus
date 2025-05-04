use std::collections::HashMap;
use std::sync::Arc;
use histogram::Histogram;
use metrics::{Counter, Gauge};
use parking_lot::RwLock;
use crate::actor::ActorPath;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct ActorMetrics {
    pub messages_processed: u64,
    pub messages_failed: u64,
    pub processing_time: Duration,
    pub mailbox_size: usize,
    pub last_processed: Option<Instant>,
}

pub struct MetricsCollector {
    actor_stats: Arc<RwLock<HashMap<ActorPath, ActorMetrics>>>,
    message_latency: Histogram,
    memory_usage: Gauge,
    error_rates: Counter,
    dead_letters: Counter,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            actor_stats: Arc::new(RwLock::new(HashMap::new())),
            message_latency: Histogram::new_with_bounds(1, 1_000_000, 3).unwrap(), // 1us to 1s
            memory_usage: metrics::gauge!("actor_system_memory_usage"),
            error_rates: metrics::counter!("actor_system_errors"),
            dead_letters: metrics::counter!("actor_system_dead_letters"),
        }
    }

    pub fn record_message_processed(&self, actor: &ActorPath, duration: Duration) {
        let mut stats = self.actor_stats.write();
        let metrics = stats.entry(actor.clone()).or_default();
        metrics.messages_processed += 1;
        metrics.processing_time += duration;
        metrics.last_processed = Some(Instant::now());
        
        self.message_latency.record(duration.as_micros() as u64);
    }

    pub fn record_message_failed(&self, actor: &ActorPath) {
        let mut stats = self.actor_stats.write();
        let metrics = stats.entry(actor.clone()).or_default();
        metrics.messages_failed += 1;
        self.error_rates.increment(1);
    }

    pub fn update_mailbox_size(&self, actor: &ActorPath, size: usize) {
        let mut stats = self.actor_stats.write();
        let metrics = stats.entry(actor.clone()).or_default();
        metrics.mailbox_size = size;
    }

    pub fn record_dead_letter(&self) {
        self.dead_letters.increment(1);
    }

    pub fn update_memory_usage(&self, bytes: i64) {
        self.memory_usage.set(bytes as f64);
    }

    pub fn get_actor_metrics(&self, actor: &ActorPath) -> Option<ActorMetrics> {
        self.actor_stats.read().get(actor).cloned()
    }

    pub fn get_latency_percentiles(&self) -> Vec<(f64, u64)> {
        vec![
            (50.0, self.message_latency.value_at_percentile(50.0)),
            (75.0, self.message_latency.value_at_percentile(75.0)),
            (90.0, self.message_latency.value_at_percentile(90.0)),
            (95.0, self.message_latency.value_at_percentile(95.0)),
            (99.0, self.message_latency.value_at_percentile(99.0)),
        ]
    }

    pub fn reset_histogram(&self) {
        self.message_latency.clear();
    }

    pub fn error_count(&self) -> u64 {
        self.error_rates.get() as u64
    }

    pub fn dead_letter_count(&self) -> u64 {
        self.dead_letters.get() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_metrics_collection() {
        let collector = MetricsCollector::new();
        let actor = ActorPath("test-actor".to_string());

        // Record some metrics
        collector.record_message_processed(&actor, Duration::from_micros(100));
        collector.record_message_processed(&actor, Duration::from_micros(200));
        collector.record_message_failed(&actor);
        collector.update_mailbox_size(&actor, 5);

        // Verify metrics
        let metrics = collector.get_actor_metrics(&actor).unwrap();
        assert_eq!(metrics.messages_processed, 2);
        assert_eq!(metrics.messages_failed, 1);
        assert_eq!(metrics.mailbox_size, 5);
        assert_eq!(metrics.processing_time, Duration::from_micros(300));

        // Check latency percentiles
        let percentiles = collector.get_latency_percentiles();
        assert!(percentiles[0].1 >= 100); // 50th percentile should be >= 100us
        assert!(percentiles[4].1 <= 200); // 99th percentile should be <= 200us

        // Test error counters
        assert_eq!(collector.error_count(), 1);
    }
}
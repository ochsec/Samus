use metrics::{counter, gauge, histogram};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// Performance monitoring metrics
#[derive(Default)]
pub struct PerformanceMetrics {
    cpu_usage: AtomicU64,
    memory_usage: AtomicU64,
    frame_time: AtomicU64,
    operation_count: AtomicU64,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_cpu_usage(&self, usage: u64) {
        self.cpu_usage.store(usage, Ordering::SeqCst);
        gauge!("cpu_usage", usage as f64);
    }

    pub fn record_memory_usage(&self, usage: u64) {
        self.memory_usage.store(usage, Ordering::SeqCst);
        gauge!("memory_usage", usage as f64);
    }

    pub fn record_frame_time(&self, duration: Duration) {
        let nanos = duration.as_nanos() as u64;
        self.frame_time.store(nanos, Ordering::SeqCst);
        histogram!("frame_time", duration.as_secs_f64());
    }

    pub fn increment_operation_count(&self) {
        self.operation_count.fetch_add(1, Ordering::SeqCst);
        counter!("operation_count", 1);
    }
}

// Benchmark runner for performance testing
pub struct BenchmarkRunner {
    metrics: Arc<PerformanceMetrics>,
    results: RwLock<HashMap<String, Vec<Duration>>>,
}

impl BenchmarkRunner {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(PerformanceMetrics::new()),
            results: RwLock::new(HashMap::new()),
        }
    }

    pub async fn run_benchmark<F, Fut>(&self, name: &str, iterations: u32, f: F) -> Vec<Duration>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let mut durations = Vec::with_capacity(iterations as usize);

        for _ in 0..iterations {
            let start = Instant::now();
            f().await;
            let duration = start.elapsed();
            durations.push(duration);

            histogram!("benchmark.duration", duration.as_secs_f64(), "name" => name.to_string());
        }

        self.results
            .write()
            .insert(name.to_string(), durations.clone());
        durations
    }

    pub fn get_statistics(&self, name: &str) -> Option<BenchmarkStats> {
        let results = self.results.read();
        results.get(name).map(|durations| {
            let mut sorted = durations.clone();
            sorted.sort();

            let total: Duration = durations.iter().sum();
            let avg = total / durations.len() as u32;
            let median = sorted[sorted.len() / 2];
            let min = sorted.first().copied().unwrap_or_default();
            let max = sorted.last().copied().unwrap_or_default();

            BenchmarkStats {
                name: name.to_string(),
                iterations: durations.len(),
                average: avg,
                median,
                min,
                max,
            }
        })
    }
}

#[derive(Debug)]
pub struct BenchmarkStats {
    pub name: String,
    pub iterations: usize,
    pub average: Duration,
    pub median: Duration,
    pub min: Duration,
    pub max: Duration,
}

// Resource usage profiles for different optimization levels
pub struct OptimizationProfile {
    pub max_memory: usize,
    pub max_cpu_usage: f64,
    pub target_frame_time: Duration,
    pub cache_size: usize,
}

impl OptimizationProfile {
    pub fn low_resource() -> Self {
        Self {
            max_memory: 100 * 1024 * 1024,                // 100MB
            max_cpu_usage: 0.3,                           // 30% CPU
            target_frame_time: Duration::from_millis(33), // ~30 FPS
            cache_size: 1024 * 1024,                      // 1MB cache
        }
    }

    pub fn balanced() -> Self {
        Self {
            max_memory: 512 * 1024 * 1024,                // 512MB
            max_cpu_usage: 0.5,                           // 50% CPU
            target_frame_time: Duration::from_millis(16), // ~60 FPS
            cache_size: 32 * 1024 * 1024,                 // 32MB cache
        }
    }

    pub fn high_performance() -> Self {
        Self {
            max_memory: 2048 * 1024 * 1024,              // 2GB
            max_cpu_usage: 0.8,                          // 80% CPU
            target_frame_time: Duration::from_millis(8), // ~120 FPS
            cache_size: 256 * 1024 * 1024,               // 256MB cache
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_benchmark_runner() {
        let runner = BenchmarkRunner::new();

        // Test async operation
        runner
            .run_benchmark("test_sleep", 3, || async {
                sleep(Duration::from_millis(10)).await;
            })
            .await;

        let stats = runner.get_statistics("test_sleep").unwrap();
        assert_eq!(stats.iterations, 3);
        assert!(stats.average >= Duration::from_millis(10));
    }

    #[test]
    fn test_performance_metrics() {
        let metrics = PerformanceMetrics::new();

        metrics.record_cpu_usage(50);
        assert_eq!(metrics.cpu_usage.load(Ordering::SeqCst), 50);

        metrics.record_memory_usage(1024);
        assert_eq!(metrics.memory_usage.load(Ordering::SeqCst), 1024);

        metrics.record_frame_time(Duration::from_millis(16));
        assert_eq!(metrics.frame_time.load(Ordering::SeqCst), 16_000_000);

        metrics.increment_operation_count();
        assert_eq!(metrics.operation_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_optimization_profiles() {
        let low = OptimizationProfile::low_resource();
        let balanced = OptimizationProfile::balanced();
        let high = OptimizationProfile::high_performance();

        assert!(low.max_memory < balanced.max_memory);
        assert!(balanced.max_memory < high.max_memory);

        assert!(low.max_cpu_usage < balanced.max_cpu_usage);
        assert!(balanced.max_cpu_usage < high.max_cpu_usage);

        assert!(low.target_frame_time > balanced.target_frame_time);
        assert!(balanced.target_frame_time > high.target_frame_time);
    }
}

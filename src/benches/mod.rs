pub mod actor_bench;
pub mod event_store_bench;
pub mod file_ops_bench;

use criterion::Criterion;
use tokio::runtime::Runtime;

// Common benchmark utilities
pub(crate) mod bench_utils {
    use super::*;
    use std::sync::Arc;
    use metrics::MetricsCollector;

    pub fn setup_runtime() -> Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    pub fn create_metrics() -> Arc<MetricsCollector> {
        Arc::new(MetricsCollector::new())
    }
}
pub mod actor_tests;
pub mod event_store_tests;
pub mod file_ops_tests;

use test_context::AsyncTestContext;
use crate::actor::ActorSystem;
use crate::cqrs::EventStore;
use crate::services::file::FileService;
use std::sync::Arc;
use metrics::MetricsCollector;

// Common test utilities and helpers
pub(crate) mod test_utils {
    use super::*;
    use tokio::runtime::Runtime;

    pub fn setup_test_runtime() -> Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    }
    
    pub fn create_test_metrics() -> Arc<MetricsCollector> {
        Arc::new(MetricsCollector::new())
    }
}
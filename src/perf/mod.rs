pub mod benchmark;
pub mod cpu;
pub mod ui;

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Re-export main types for easier access

// Memory tracking
pub struct MemoryStats {
    allocated: AtomicUsize,
    peak: AtomicUsize,
    buffers_in_use: AtomicUsize,
}

impl MemoryStats {
    pub fn new() -> Self {
        Self {
            allocated: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            buffers_in_use: AtomicUsize::new(0),
        }
    }

    pub fn record_allocation(&self, size: usize) {
        let current = self.allocated.fetch_add(size, Ordering::SeqCst);
        let new_total = current + size;
        let mut peak = self.peak.load(Ordering::SeqCst);
        while new_total > peak {
            match self
                .peak
                .compare_exchange(peak, new_total, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
    }

    pub fn record_deallocation(&self, size: usize) {
        self.allocated.fetch_sub(size, Ordering::SeqCst);
    }
}

// Buffer pooling
pub struct BufferPool {
    pools: RwLock<HashMap<usize, Vec<Vec<u8>>>>,
    stats: Arc<MemoryStats>,
}

impl BufferPool {
    pub fn new(stats: Arc<MemoryStats>) -> Self {
        Self {
            pools: RwLock::new(HashMap::new()),
            stats,
        }
    }

    pub fn acquire(&self, size: usize) -> Vec<u8> {
        let mut pools = self.pools.write();
        if let Some(pool) = pools.get_mut(&size) {
            if let Some(buffer) = pool.pop() {
                self.stats.buffers_in_use.fetch_add(1, Ordering::SeqCst);
                return buffer;
            }
        }
        self.stats.record_allocation(size);
        self.stats.buffers_in_use.fetch_add(1, Ordering::SeqCst);
        Vec::with_capacity(size)
    }

    pub fn release(&self, mut buffer: Vec<u8>) {
        let size = buffer.capacity();
        buffer.clear();
        let mut pools = self.pools.write();
        let pool = pools.entry(size).or_insert_with(Vec::new);
        pool.push(buffer);
        self.stats.buffers_in_use.fetch_sub(1, Ordering::SeqCst);
    }
}

// Resource cleanup
pub struct ResourceTracker {
    resources: RwLock<HashMap<String, Box<dyn FnOnce() + Send + 'static>>>,
}

impl ResourceTracker {
    pub fn new() -> Self {
        Self {
            resources: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<F>(&self, id: String, cleanup: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let mut resources = self.resources.write();
        resources.insert(id, Box::new(cleanup));
    }

    pub fn cleanup(&self, id: &str) {
        let mut resources = self.resources.write();
        if let Some(cleanup) = resources.remove(id) {
            cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stats() {
        let stats = MemoryStats::new();
        stats.record_allocation(1000);
        assert_eq!(stats.allocated.load(Ordering::SeqCst), 1000);
        assert_eq!(stats.peak.load(Ordering::SeqCst), 1000);

        stats.record_allocation(500);
        assert_eq!(stats.allocated.load(Ordering::SeqCst), 1500);
        assert_eq!(stats.peak.load(Ordering::SeqCst), 1500);

        stats.record_deallocation(1000);
        assert_eq!(stats.allocated.load(Ordering::SeqCst), 500);
        assert_eq!(stats.peak.load(Ordering::SeqCst), 1500);
    }

    #[test]
    fn test_buffer_pool() {
        let stats = Arc::new(MemoryStats::new());
        let pool = BufferPool::new(Arc::clone(&stats));

        let buf1 = pool.acquire(1024);
        assert_eq!(buf1.capacity(), 1024);
        assert_eq!(stats.buffers_in_use.load(Ordering::SeqCst), 1);

        pool.release(buf1);
        assert_eq!(stats.buffers_in_use.load(Ordering::SeqCst), 0);

        let buf2 = pool.acquire(1024);
        assert_eq!(buf2.capacity(), 1024);
    }

    #[test]
    fn test_resource_tracker() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let tracker = ResourceTracker::new();
        let cleaned_up = Arc::new(AtomicBool::new(false));
        let cleaned_up_clone = Arc::clone(&cleaned_up);

        tracker.register("test".to_string(), move || {
            cleaned_up_clone.store(true, Ordering::SeqCst);
        });

        tracker.cleanup("test");
        assert!(cleaned_up.load(Ordering::SeqCst));
    }
}

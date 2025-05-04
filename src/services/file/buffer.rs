use std::sync::Arc;
use object_pool::{Pool, Reusable};
use parking_lot::Mutex;

const DEFAULT_BUFFER_SIZE: usize = 64 * 1024; // 64KB default buffer size
const MIN_BUFFER_SIZE: usize = 4 * 1024;      // 4KB minimum
const MAX_BUFFER_SIZE: usize = 1024 * 1024;   // 1MB maximum

#[derive(Debug)]
pub struct Buffer {
    data: Vec<u8>,
    size: usize,
}

impl Buffer {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
            size,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.size]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data[..self.size]
    }

    pub fn resize(&mut self, new_size: usize) {
        self.data.resize(new_size, 0);
        self.size = new_size;
    }
}

pub struct BufferPool {
    small_pool: Pool<Buffer>,  // For small files (<64KB)
    medium_pool: Pool<Buffer>, // For medium files (64KB-256KB)
    large_pool: Pool<Buffer>,  // For large files (>256KB)
}

impl BufferPool {
    pub fn new(small_count: usize, medium_count: usize, large_count: usize) -> Arc<Self> {
        Arc::new(Self {
            small_pool: Pool::new(small_count, || Buffer::new(MIN_BUFFER_SIZE)),
            medium_pool: Pool::new(medium_count, || Buffer::new(DEFAULT_BUFFER_SIZE)),
            large_pool: Pool::new(large_count, || Buffer::new(MAX_BUFFER_SIZE)),
        })
    }

    pub fn acquire(&self, size: usize) -> Reusable<Buffer> {
        if size <= MIN_BUFFER_SIZE {
            self.small_pool.try_pull().unwrap_or_else(|| Buffer::new(MIN_BUFFER_SIZE))
        } else if size <= DEFAULT_BUFFER_SIZE {
            self.medium_pool.try_pull().unwrap_or_else(|| Buffer::new(DEFAULT_BUFFER_SIZE))
        } else {
            self.large_pool.try_pull().unwrap_or_else(|| Buffer::new(MAX_BUFFER_SIZE))
        }
    }

    pub fn metrics(&self) -> BufferPoolMetrics {
        BufferPoolMetrics {
            small_available: self.small_pool.available(),
            medium_available: self.medium_pool.available(),
            large_available: self.large_pool.available(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BufferPoolMetrics {
    pub small_available: usize,
    pub medium_available: usize,
    pub large_available: usize,
}
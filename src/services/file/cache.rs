use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use lru::LruCache;
use parking_lot::Mutex;

#[derive(Clone)]
pub struct CacheEntry {
    pub data: Arc<Vec<u8>>,
    pub last_modified: Instant,
    pub ttl: Duration,
}

impl CacheEntry {
    pub fn is_expired(&self) -> bool {
        self.last_modified.elapsed() > self.ttl
    }
}

pub struct AsyncCache<K, V> {
    cache: Arc<Mutex<LruCache<K, V>>>,
    max_size: usize,
}

impl<K: Clone + Eq + std::hash::Hash, V> AsyncCache<K, V> {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(max_size))),
            max_size,
        }
    }

    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        self.cache.lock().get(key).cloned()
    }

    pub fn insert(&self, key: K, value: V) -> Option<V> {
        self.cache.lock().put(key, value)
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        self.cache.lock().pop(key)
    }

    pub fn clear(&self) {
        self.cache.lock().clear();
    }
}

pub struct CacheConfig {
    pub max_size: usize,
    pub default_ttl: Duration,
    pub eviction_policy: EvictionPolicy,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            default_ttl: Duration::from_secs(300), // 5 minutes
            eviction_policy: EvictionPolicy::LRU,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum EvictionPolicy {
    LRU,
    FIFO,
    Random,
}

pub struct FileCache {
    cache: Arc<AsyncCache<PathBuf, CacheEntry>>,
    config: CacheConfig,
}

impl FileCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(AsyncCache::new(config.max_size)),
            config,
        }
    }

    pub fn get(&self, path: &Path) -> Option<CacheEntry> {
        if let Some(entry) = self.cache.get(&path.to_path_buf()) {
            if !entry.is_expired() {
                return Some(entry);
            }
            // Remove expired entry
            self.cache.remove(&path.to_path_buf());
        }
        None
    }

    pub fn insert(&self, path: PathBuf, data: Vec<u8>) {
        let entry = CacheEntry {
            data: Arc::new(data),
            last_modified: Instant::now(),
            ttl: self.config.default_ttl,
        };
        self.cache.insert(path, entry);
    }

    pub fn invalidate(&self, path: &Path) {
        self.cache.remove(&path.to_path_buf());
    }

    pub fn clear(&self) {
        self.cache.clear();
    }

    pub fn set_ttl(&self, path: &Path, ttl: Duration) {
        if let Some(mut entry) = self.cache.get(&path.to_path_buf()) {
            entry.ttl = ttl;
            self.cache.insert(path.to_path_buf(), entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_cache_operations() {
        let config = CacheConfig {
            max_size: 2,
            default_ttl: Duration::from_millis(100),
            eviction_policy: EvictionPolicy::LRU,
        };
        let cache = FileCache::new(config);

        let path1 = PathBuf::from("test1.txt");
        let path2 = PathBuf::from("test2.txt");
        let path3 = PathBuf::from("test3.txt");

        // Test insertion and retrieval
        cache.insert(path1.clone(), vec![1, 2, 3]);
        assert_eq!(cache.get(&path1).unwrap().data.as_ref(), &vec![1, 2, 3]);

        // Test LRU eviction
        cache.insert(path2.clone(), vec![4, 5, 6]);
        cache.insert(path3.clone(), vec![7, 8, 9]);
        assert!(cache.get(&path1).is_none()); // path1 should be evicted

        // Test TTL expiration
        thread::sleep(Duration::from_millis(150));
        assert!(cache.get(&path2).is_none()); // Should be expired
        assert!(cache.get(&path3).is_none()); // Should be expired

        // Test invalidation
        cache.insert(path1.clone(), vec![1, 2, 3]);
        cache.invalidate(&path1);
        assert!(cache.get(&path1).is_none());
    }
}
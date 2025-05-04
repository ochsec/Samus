use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// Frame rate limiter for UI rendering
pub struct FrameLimiter {
    target_fps: u32,
    frame_duration: Duration,
    last_frame: RwLock<Instant>,
}

impl FrameLimiter {
    pub fn new(target_fps: u32) -> Self {
        Self {
            target_fps,
            frame_duration: Duration::from_secs(1) / target_fps,
            last_frame: RwLock::new(Instant::now() - Duration::from_secs(1)), // Initialize to force first frame
        }
    }

    pub fn should_render(&self) -> bool {
        let now = Instant::now();
        let mut last_frame = self.last_frame.write();
        let elapsed = now.duration_since(*last_frame);

        if elapsed >= self.frame_duration {
            *last_frame = now;
            true
        } else {
            false
        }
    }
}

// Widget caching system
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct WidgetCacheKey {
    widget_id: String,
    data_hash: u64,
}

pub struct CachedWidget {
    content: Vec<u8>,
    created_at: Instant,
    ttl: Duration,
}

pub struct WidgetCache {
    cache: RwLock<HashMap<WidgetCacheKey, CachedWidget>>,
    max_size: usize,
}

impl WidgetCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::with_capacity(max_size)),
            max_size,
        }
    }

    pub fn get(&self, key: &WidgetCacheKey) -> Option<Vec<u8>> {
        let cache = self.cache.read();
        cache.get(key).and_then(|widget| {
            if widget.created_at.elapsed() < widget.ttl {
                Some(widget.content.clone())
            } else {
                None
            }
        })
    }

    pub fn set(&self, key: WidgetCacheKey, content: Vec<u8>, ttl: Duration) {
        let mut cache = self.cache.write();

        // Clean up expired entries if cache is full
        if cache.len() >= self.max_size {
            let _now = Instant::now();
            cache.retain(|_, widget| widget.created_at.elapsed() < widget.ttl);

            // If still full, remove oldest entries
            if cache.len() >= self.max_size {
                let mut entries: Vec<_> =
                    cache.iter().map(|(k, v)| (k.clone(), v)).collect();
                entries.sort_by_key(|(_, v)| v.created_at);
                let to_remove = entries.len() - self.max_size + 1;

                // Collect keys first, then remove
                let keys_to_remove: Vec<_> = entries
                    .iter()
                    .take(to_remove)
                    .map(|(k, _)| k)
                    .cloned()
                    .collect();

                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }
        }

        cache.insert(
            key,
            CachedWidget {
                content,
                created_at: Instant::now(),
                ttl,
            },
        );
    }
}

// Partial screen update tracker
pub struct DirtyRegionTracker {
    regions: RwLock<Vec<(usize, usize, usize, usize)>>, // x, y, width, height
}

impl DirtyRegionTracker {
    pub fn new() -> Self {
        Self {
            regions: RwLock::new(Vec::new()),
        }
    }

    pub fn mark_dirty(&self, x: usize, y: usize, width: usize, height: usize) {
        let mut regions = self.regions.write();
        regions.push((x, y, width, height));
    }

    pub fn get_dirty_regions(&self) -> Vec<(usize, usize, usize, usize)> {
        let regions = self.regions.read();
        regions.clone()
    }

    pub fn clear(&self) {
        let mut regions = self.regions.write();
        regions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_frame_limiter() {
        let limiter = FrameLimiter::new(30); // Use lower FPS for more reliable timing
        
        // First frame should always render because we initialized last_frame in the past
        assert!(limiter.should_render(), "First frame should render");
        
        // Second frame should not render immediately
        assert!(!limiter.should_render(), "Frame should not render before interval");
        
        // Wait for frame duration (33.33ms at 30 FPS) plus a small buffer
        thread::sleep(Duration::from_millis(40));
        assert!(limiter.should_render(), "Frame should render after interval");
        
        // Immediate frame after should not render
        assert!(!limiter.should_render(), "Frame should not render immediately after previous frame");
        
        // Wait again and verify we can render
        thread::sleep(Duration::from_millis(40));
        assert!(limiter.should_render(), "Frame should render after second interval");
    }

    #[test]
    fn test_widget_cache() {
        let cache = WidgetCache::new(2);
        let key1 = WidgetCacheKey {
            widget_id: "widget1".to_string(),
            data_hash: 123,
        };
        let key2 = WidgetCacheKey {
            widget_id: "widget2".to_string(),
            data_hash: 456,
        };

        cache.set(key1.clone(), vec![1, 2, 3], Duration::from_secs(1));
        cache.set(key2.clone(), vec![4, 5, 6], Duration::from_secs(1));

        assert_eq!(cache.get(&key1), Some(vec![1, 2, 3]));
        assert_eq!(cache.get(&key2), Some(vec![4, 5, 6]));

        // Test max size enforcement
        let key3 = WidgetCacheKey {
            widget_id: "widget3".to_string(),
            data_hash: 789,
        };
        cache.set(key3.clone(), vec![7, 8, 9], Duration::from_secs(1));

        // Oldest entry should be evicted
        assert_eq!(cache.get(&key1), None);
        assert_eq!(cache.get(&key2), Some(vec![4, 5, 6]));
        assert_eq!(cache.get(&key3), Some(vec![7, 8, 9]));
    }

    #[test]
    fn test_dirty_region_tracker() {
        let tracker = DirtyRegionTracker::new();
        tracker.mark_dirty(0, 0, 10, 10);
        tracker.mark_dirty(20, 20, 5, 5);

        let regions = tracker.get_dirty_regions();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0], (0, 0, 10, 10));
        assert_eq!(regions[1], (20, 20, 5, 5));

        tracker.clear();
        assert!(tracker.get_dirty_regions().is_empty());
    }
}

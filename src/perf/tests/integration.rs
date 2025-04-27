use super::*;
use tokio::time::sleep;
use std::time::Duration;

#[tokio::test]
async fn test_integrated_optimizations() {
    // Initialize performance components
    let metrics = Arc::new(PerformanceMetrics::new());
    let buffer_pool = Arc::new(BufferPool::new(Arc::clone(&metrics)));
    let resource_tracker = Arc::new(ResourceTracker::new());
    let task_scheduler = Arc::new(TaskScheduler::new(4));
    let widget_cache = Arc::new(WidgetCache::new(1000));
    let frame_limiter = Arc::new(FrameLimiter::new(60));
    let benchmark_runner = Arc::new(BenchmarkRunner::new());

    // Test buffer pooling
    {
        let buf1 = buffer_pool.acquire(1024);
        assert_eq!(buf1.capacity(), 1024);
        buffer_pool.release(buf1);
        
        let buf2 = buffer_pool.acquire(1024);
        assert_eq!(buf2.capacity(), 1024);
        buffer_pool.release(buf2);
    }

    // Test task scheduling and execution
    let completion_counter = Arc::new(AtomicU64::new(0));
    {
        let counter = Arc::clone(&completion_counter);
        let task = Task {
            id: "test_task".to_string(),
            priority: Priority::High,
            work: Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        };
        task_scheduler.schedule(task);
    }

    // Test resource tracking
    {
        let cleanup_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&cleanup_flag);
        
        resource_tracker.register("test_resource".to_string(), move || {
            flag_clone.store(true, Ordering::SeqCst);
        });
        
        resource_tracker.cleanup("test_resource");
        assert!(cleanup_flag.load(Ordering::SeqCst));
    }

    // Test widget caching
    {
        let key = WidgetCacheKey {
            widget_id: "test_widget".to_string(),
            data_hash: 12345,
        };
        
        let content = vec![1, 2, 3, 4, 5];
        widget_cache.set(key.clone(), content.clone(), Duration::from_secs(1));
        
        let cached = widget_cache.get(&key);
        assert_eq!(cached, Some(content));
    }

    // Test frame limiting
    {
        assert!(frame_limiter.should_render()); // First frame
        assert!(!frame_limiter.should_render()); // Too soon
        sleep(Duration::from_millis(17)).await;
        assert!(frame_limiter.should_render()); // After delay
    }

    // Test performance benchmarking
    {
        let durations = benchmark_runner.run_benchmark("test_operation", 5, || async {
            sleep(Duration::from_millis(1)).await;
        }).await;
        
        assert_eq!(durations.len(), 5);
        
        let stats = benchmark_runner.get_statistics("test_operation").unwrap();
        assert!(stats.average >= Duration::from_millis(1));
        assert!(stats.max >= stats.average);
        assert!(stats.min <= stats.average);
    }

    // Test optimization profiles
    {
        let profile = OptimizationProfile::balanced();
        
        // Verify memory limits are respected
        let large_buf = buffer_pool.acquire(profile.max_memory / 2);
        assert!(large_buf.capacity() <= profile.max_memory);
        buffer_pool.release(large_buf);
        
        // Verify frame timing
        assert!(frame_limiter.should_render());
        sleep(profile.target_frame_time).await;
        assert!(frame_limiter.should_render());
    }

    // Test metrics recording
    {
        metrics.record_cpu_usage(50);
        metrics.record_memory_usage(1024 * 1024);
        metrics.record_frame_time(Duration::from_millis(16));
        metrics.increment_operation_count();
        
        assert!(metrics.cpu_usage.load(Ordering::SeqCst) > 0);
        assert!(metrics.memory_usage.load(Ordering::SeqCst) > 0);
        assert!(metrics.frame_time.load(Ordering::SeqCst) > 0);
        assert!(metrics.operation_count.load(Ordering::SeqCst) > 0);
    }
}
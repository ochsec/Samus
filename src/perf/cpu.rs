use std::sync::Arc;
use parking_lot::RwLock;
use cached::proc_macro::cached;
use tokio::sync::mpsc;
use std::collections::{HashMap, VecDeque};
use metrics::{counter, gauge};

// Task priority levels
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

// Task representation
pub struct Task {
    id: String,
    priority: Priority,
    work: Box<dyn FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + 'static>,
}

// Task scheduler for CPU optimization
pub struct TaskScheduler {
    queues: RwLock<HashMap<Priority, VecDeque<Task>>>,
    max_concurrent: usize,
    active_tasks: Arc<RwLock<usize>>,
}

impl TaskScheduler {
    pub fn new(max_concurrent: usize) -> Self {
        let mut queues = HashMap::new();
        for priority in [Priority::Low, Priority::Normal, Priority::High, Priority::Critical].iter() {
            queues.insert(*priority, VecDeque::new());
        }

        Self {
            queues: RwLock::new(queues),
            max_concurrent,
            active_tasks: Arc::new(RwLock::new(0)),
        }
    }

    pub fn schedule(&self, task: Task) {
        let mut queues = self.queues.write();
        queues.get_mut(&task.priority)
            .expect("Invalid priority")
            .push_back(task);
        
        counter!("scheduled_tasks", 1);
    }

    pub async fn run(&self) {
        let (tx, mut rx) = mpsc::channel(100);
        
        loop {
            if *self.active_tasks.read() < self.max_concurrent {
                let task = self.get_next_task();
                if let Some(task) = task {
                    let tx = tx.clone();
                    let active_tasks = Arc::clone(&self.active_tasks);
                    
                    *active_tasks.write() += 1;
                    gauge!("active_tasks", *active_tasks.read() as f64);
                    
                    tokio::spawn(async move {
                        let result = (task.work)();
                        *active_tasks.write() -= 1;
                        gauge!("active_tasks", *active_tasks.read() as f64);
                        let _ = tx.send(result).await;
                    });
                }
            }
            
            if let Some(result) = rx.recv().await {
                if let Err(e) = result {
                    eprintln!("Task error: {}", e);
                    counter!("task_errors", 1);
                }
            }
        }
    }

    fn get_next_task(&self) -> Option<Task> {
        let mut queues = self.queues.write();
        for priority in [Priority::Critical, Priority::High, Priority::Normal, Priority::Low].iter() {
            if let Some(task) = queues.get_mut(priority).unwrap().pop_front() {
                return Some(task);
            }
        }
        None
    }
}

// Lazy loading implementation
pub struct LazyLoader<T, F>
where
    F: FnOnce() -> T,
{
    value: RwLock<Option<T>>,
    init: Option<F>,
}

impl<T, F> LazyLoader<T, F>
where
    F: FnOnce() -> T,
{
    pub fn new(init: F) -> Self {
        Self {
            value: RwLock::new(None),
            init: Some(init),
        }
    }

    pub fn get(&mut self) -> &T {
        // Initialize the value if it hasn't been initialized yet
        if self.value.read().is_none() {
            // Check if we need to initialize
            let should_init = self.value.read().is_none() && self.init.is_some();
            
            // If we need to initialize, take the initializer function
            let init_fn = if should_init {
                self.init.take()
            } else {
                None
            };
            
            // Call the initializer if we took it
            if let Some(initializer) = init_fn {
                let result = initializer();
                *self.value.write() = Some(result);
            }
        }
        
        // The value exists now, get a read guard and return a reference
        let guard = self.value.read();
        let value_ref = guard.as_ref().unwrap();
        
        // SAFETY: We're extending the lifetime of the reference beyond the guard
        // This is safe as long as self outlives the returned reference
        unsafe {
            let ptr = value_ref as *const T;
            &*ptr
        }
    }
}

// Background task manager
pub struct BackgroundTaskManager {
    scheduler: Arc<TaskScheduler>,
}

impl BackgroundTaskManager {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            scheduler: Arc::new(TaskScheduler::new(max_concurrent)),
        }
    }

    pub fn spawn<F>(&self, priority: Priority, work: F)
    where
        F: FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
    {
        self.scheduler.schedule(Task {
            id: uuid::Uuid::new_v4().to_string(),
            priority,
            work: Box::new(work),
        });
    }

    pub async fn run(&self) {
        self.scheduler.run().await;
    }
}

// Function result caching
#[cached(
    type = "cached::SizedCache<String, Vec<u8>>",
    create = "{ cached::SizedCache::with_size(100) }",
    convert = r#"{ format!("{:?}-{:?}", _path, _options) }"#
)]
pub async fn cached_file_read(_path: String, _options: HashMap<String, String>) -> Vec<u8> {
    // Simulated expensive file read
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    vec![] // Placeholder for actual file read
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_task_scheduler() {
        let scheduler = TaskScheduler::new(2);
        let counter = Arc::new(AtomicUsize::new(0));
        
        for i in 0..5 {
            let counter = Arc::clone(&counter);
            scheduler.schedule(Task {
                id: i.to_string(),
                priority: Priority::Normal,
                work: Box::new(move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }),
            });
        }
        
        tokio::spawn(async move {
            scheduler.run().await;
        });
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(counter.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_lazy_loader() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        
        let lazy = LazyLoader::new(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            42
        });
        
        assert_eq!(*lazy.get(), 42);
        assert_eq!(*lazy.get(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
use cached::proc_macro::cached;
use metrics::{counter, gauge};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::mpsc;

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
    work: Box<dyn FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync + 'static>,
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
        for priority in [
            Priority::Low,
            Priority::Normal,
            Priority::High,
            Priority::Critical,
        ]
        .iter()
        {
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
        queues
            .get_mut(&task.priority)
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
        for priority in [
            Priority::Critical,
            Priority::High,
            Priority::Normal,
            Priority::Low,
        ]
        .iter()
        {
            if let Some(task) = queues.get_mut(priority).unwrap().pop_front() {
                return Some(task);
            }
        }
        None
    }
}

// Lazy loading implementation
struct LazyState<T, F> {
    value: Option<T>,
    init: Option<F>,
}

pub struct LazyLoader<T, F>
where
    F: FnOnce() -> T,
{
    state: RwLock<LazyState<T, F>>,
}

impl<T, F> LazyLoader<T, F>
where
    F: FnOnce() -> T,
{
    pub fn new(init: F) -> Self {
        Self {
            state: RwLock::new(LazyState {
                value: None,
                init: Some(init),
            }),
        }
    }

    pub fn get(&self) -> &T {
        // Try read-only access first
        {
            let state = self.state.read();
            if let Some(value) = &state.value {
                return unsafe {
                    // SAFETY: Value exists and won't be modified while we hold the read lock
                    let ptr = value as *const T;
                    &*ptr
                };
            }
        }

        // Need to initialize - acquire write lock
        let mut state = self.state.write();
        
        // Check again in case another thread initialized while we were waiting
        if state.value.is_none() {
            if let Some(init) = state.init.take() {
                state.value = Some(init());
            }
        }

        unsafe {
            // SAFETY: Value is now initialized and won't be modified while we hold the lock
            let ptr = state.value.as_ref().unwrap() as *const T;
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
        F: FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync + 'static,
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
        let scheduler = Arc::new(TaskScheduler::new(2));
        let counter = Arc::new(AtomicUsize::new(0));

        // Spawn the scheduler in a separate task
        let scheduler_clone = Arc::clone(&scheduler);
        let scheduler_handle = tokio::spawn(async move {
            scheduler_clone.run().await;
        });

        // Schedule some test tasks
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

        // Wait a bit for tasks to process
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Verify tasks were processed
        assert!(counter.load(Ordering::SeqCst) > 0);
        
        // Clean up
        scheduler_handle.abort();
    }

    #[test]
    fn test_lazy_loader() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let lazy = LazyLoader::new(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            42
        });

        // First access should initialize
        let val1 = lazy.get();
        assert_eq!(*val1, 42);
        
        // Second access should use cached value
        let val2 = lazy.get();
        assert_eq!(*val2, 42);
        
        // Verify initialization only happened once
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}

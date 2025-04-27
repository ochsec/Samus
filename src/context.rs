use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A token that can be used to cancel an operation.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    is_cancelled: Arc<RwLock<bool>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        CancellationToken {
            is_cancelled: Arc::new(RwLock::new(false)),
        }
    }

    pub fn cancel(&self) {
        if let Ok(mut cancelled) = self.is_cancelled.write() {
            *cancelled = true;
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled.read().map(|c| *c).unwrap_or(false)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// The execution context for a task.
#[derive(Debug)]
pub struct Context {
    values: RwLock<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            values: RwLock::new(HashMap::new()),
        }
    }

    pub fn set<T: 'static + Send + Sync>(&self, key: &str, value: T) {
        if let Ok(mut map) = self.values.write() {
            map.insert(key.to_string(), Box::new(value));
        }
    }

    pub fn get<T: 'static + Send + Sync + Clone>(&self, key: &str) -> Option<T> {
        if let Ok(map) = self.values.read() {
            if let Some(value) = map.get(key) {
                if let Some(typed_value) = value.downcast_ref::<T>() {
                    return Some(typed_value.clone());
                }
            }
        }
        None
    }

    pub fn contains_key(&self, key: &str) -> bool {
        if let Ok(map) = self.values.read() {
            return map.contains_key(key);
        }
        false
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

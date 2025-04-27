use std::any::Any;
use std::sync::Arc;

/// A trait representing a resource that can be used by a task.
pub trait Resource: Send + Sync + 'static {
    /// Get the unique identifier for this resource.
    fn id(&self) -> &str;

    /// Get the resource type name.
    fn resource_type(&self) -> &str;

    /// Get a reference to the underlying resource as Any.
    fn as_any(&self) -> &dyn Any;

    /// Get a mutable reference to the underlying resource as Any.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub type ResourceRef = Arc<dyn Resource>;

// Wrapper for ResourceRef to implement Debug
#[derive(Clone)]
pub struct ResourceRefWrapper(pub ResourceRef);

impl std::fmt::Debug for ResourceRefWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceRef")
            .field("id", &self.0.id())
            .field("type", &self.0.resource_type())
            .finish()
    }
}

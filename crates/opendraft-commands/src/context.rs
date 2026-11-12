//! Shared values a running command can reach, handed to it at launch by
//! whoever starts it. The command library knows nothing of their types: it
//! only carries them, keyed by type, so an application can give its commands
//! state they share (such as the last values used) without a global.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// Values keyed by their type, shared with every command started with it.
#[derive(Clone, Default)]
pub struct Context {
    items: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Context {
    /// This context with `value` added, replacing any of the same type.
    #[must_use]
    pub fn with<T: Send + Sync + 'static>(mut self, value: Arc<T>) -> Self {
        self.items.insert(TypeId::of::<T>(), value);
        self
    }

    /// The value of type `T`, if one was given.
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        let item = Arc::clone(self.items.get(&TypeId::of::<T>())?);
        item.downcast().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_found_by_its_type() {
        let context = Context::default()
            .with(Arc::new(7_u32))
            .with(Arc::new("seven".to_owned()));
        assert_eq!(context.get::<u32>().as_deref(), Some(&7));
        assert_eq!(
            context.get::<String>().as_deref().map(String::as_str),
            Some("seven")
        );
        assert!(context.get::<f64>().is_none());
    }

    #[test]
    fn a_later_value_of_the_same_type_replaces_the_earlier() {
        let context = Context::default()
            .with(Arc::new(1_u32))
            .with(Arc::new(2_u32));
        assert_eq!(context.get::<u32>().as_deref(), Some(&2));
    }

    #[test]
    fn clones_share_the_same_values() {
        let shared = Arc::new(std::sync::Mutex::new(0));
        let context = Context::default().with(Arc::clone(&shared));
        let copy = context.clone();
        *copy.get::<std::sync::Mutex<i32>>().unwrap().lock().unwrap() = 5;
        assert_eq!(*shared.lock().unwrap(), 5);
    }
}

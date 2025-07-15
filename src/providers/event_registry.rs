//! # Event Registry
//!
//! Central registry for dynamic event registration and dispatch

use std::collections::HashMap;
use std::sync::Arc;
use serde_json::Value;
use tokio::sync::RwLock;

/// Event handler type
pub type EventHandler = Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>;

/// Central event registry for dynamic registration
pub struct EventRegistry {
    /// Map of event name to handler
    handlers: RwLock<HashMap<String, EventHandler>>,
}

impl EventRegistry {
    /// Create a new event registry
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register an event handler
    pub async fn register(&self, event_name: &str, handler: EventHandler) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(event_name.to_string(), handler);
        println!("✅ Registered event handler: {}", event_name);
    }

    /// Execute an event by name
    pub async fn execute(&self, event_name: &str, data: Value) -> Result<Value, String> {
        let handlers = self.handlers.read().await;
        
        if let Some(handler) = handlers.get(event_name) {
            handler(data)
        } else {
            Err(format!("Event '{}' not found", event_name))
        }
    }

    /// List all registered events
    pub async fn list_events(&self) -> Vec<String> {
        let handlers = self.handlers.read().await;
        handlers.keys().cloned().collect()
    }

    /// Check if an event is registered
    pub async fn has_event(&self, event_name: &str) -> bool {
        let handlers = self.handlers.read().await;
        handlers.contains_key(event_name)
    }
}

impl Default for EventRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for objects that can register with the event registry
pub trait EventRegistryClient {
    fn register(&mut self, event_name: &str, handler: EventHandler);
}

/// Implementation of EventRegistryClient for EventRegistry
impl EventRegistryClient for EventRegistry {
    fn register(&mut self, event_name: &str, handler: EventHandler) {
        // This is a sync version for use during initialization
        // We need to use the async version internally
        let rt = tokio::runtime::Handle::current();
        let event_name = event_name.to_string();
        rt.block_on(async {
            let mut handlers = self.handlers.write().await;
            handlers.insert(event_name.clone(), handler);
            println!("✅ Registered event handler: {}", event_name);
        });
    }
}
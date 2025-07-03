//! # Event System
//!
//! Type-safe event system with O(1) lookup and compile-time guarantees.
//! All plugin communication happens through events.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

/// Core event trait that all events must implement
pub trait Event: Send + Sync + Any + Debug {
    fn type_name() -> &'static str where Self: Sized;
    fn serialize(&self) -> Result<Vec<u8>, EventError>;
    fn deserialize(data: &[u8]) -> Result<Self, EventError> where Self: Sized;
    fn as_any(&self) -> &dyn Any;
}

/// Generic event handler trait for type erasure
#[async_trait]
pub trait EventHandler: Send + Sync + Debug {
    async fn handle(&self, data: &[u8]) -> Result<(), EventError>;
    fn expected_type_id(&self) -> TypeId;
    fn handler_name(&self) -> &str;
}

/// Typed event handler implementation
pub struct TypedEventHandler<T, F>
where
    T: Event + 'static,
    F: Fn(T) -> Result<(), EventError> + Send + Sync + 'static,
{
    handler_name: String,
    handler_fn: F,
    _phantom: std::marker::PhantomData<T>,
}

// Manual Debug implementation since F usually doesn't implement Debug
impl<T, F> std::fmt::Debug for TypedEventHandler<T, F>
where
    T: Event + 'static,
    F: Fn(T) -> Result<(), EventError> + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedEventHandler")
            .field("handler_name", &self.handler_name)
            .field("_phantom", &self._phantom)
            .finish()
    }
}

impl<T, F> TypedEventHandler<T, F>
where
    T: Event + 'static,
    F: Fn(T) -> Result<(), EventError> + Send + Sync + 'static,
{
    pub fn new(handler_name: String, handler_fn: F) -> Self {
        Self {
            handler_name,
            handler_fn,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<T, F> EventHandler for TypedEventHandler<T, F>
where
    T: Event + 'static,
    F: Fn(T) -> Result<(), EventError> + Send + Sync + 'static,
{
    async fn handle(&self, data: &[u8]) -> Result<(), EventError> {
        let event = T::deserialize(data)?;
        (self.handler_fn)(event)
    }

    fn expected_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn handler_name(&self) -> &str {
        &self.handler_name
    }
}

/// Main event system for managing event handlers and emission
#[derive(Debug)]
pub struct EventSystem {
    /// Map of event keys to handlers
    handlers: RwLock<HashMap<String, Vec<Arc<dyn EventHandler>>>>,
    /// Event system statistics
    stats: RwLock<EventSystemStats>,
}

impl EventSystem {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            stats: RwLock::new(EventSystemStats::default()),
        }
    }

    /// Register an event handler for a specific event type and key
    pub async fn on_event<T, F>(&self, event_key: &str, handler: F) -> Result<(), EventError>
    where
        T: Event + 'static,
        F: Fn(T) -> Result<(), EventError> + Send + Sync + 'static,
    {
        let handler_name = format!("{}::{}", event_key, T::type_name());
        let typed_handler = TypedEventHandler::new(handler_name, handler);
        
        let mut handlers = self.handlers.write().await;
        handlers.entry(event_key.to_string())
            .or_insert_with(Vec::new)
            .push(Arc::new(typed_handler));
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_handlers += 1;
        
        Ok(())
    }

    /// Emit an event to all registered handlers
    pub async fn emit_event<T>(&self, event_key: &str, event: &T) -> Result<(), EventError>
    where
        T: Event,
    {
        let data = event.serialize()?;
        let handlers = self.handlers.read().await;

        if let Some(event_handlers) = handlers.get(event_key) {
            for handler in event_handlers {
                if let Err(e) = handler.handle(&data).await {
                    // Log error but continue processing other handlers
                    eprintln!("Handler {} failed: {}", handler.handler_name(), e);
                }
            }
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.events_emitted += 1;
        
        Ok(())
    }

    /// Get current event system statistics
    pub async fn get_stats(&self) -> EventSystemStats {
        self.stats.read().await.clone()
    }
}

/// Event system statistics
#[derive(Debug, Default, Clone)]
pub struct EventSystemStats {
    pub total_handlers: usize,
    pub events_emitted: u64,
}

/// Event system errors
#[derive(Error, Debug)]
pub enum EventError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Handler execution error: {0}")]
    HandlerExecution(String),
}

/// Core system events for plugin lifecycle

/// Event emitted when a plugin connects to the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConnectedEvent {
    pub plugin_name: String,
    pub declared_features: Vec<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Event for PluginConnectedEvent {
    fn type_name() -> &'static str {
        "PluginConnectedEvent"
    }

    fn serialize(&self) -> Result<Vec<u8>, EventError> {
        serde_json::to_vec(self).map_err(EventError::Serialization)
    }

    fn deserialize(data: &[u8]) -> Result<Self, EventError> {
        serde_json::from_slice(data).map_err(EventError::Serialization)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Event emitted when a plugin disconnects from the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDisconnectedEvent {
    pub plugin_name: String,
    pub reason: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Event for PluginDisconnectedEvent {
    fn type_name() -> &'static str {
        "PluginDisconnectedEvent"
    }

    fn serialize(&self) -> Result<Vec<u8>, EventError> {
        serde_json::to_vec(self).map_err(EventError::Serialization)
    }

    fn deserialize(data: &[u8]) -> Result<Self, EventError> {
        serde_json::from_slice(data).map_err(EventError::Serialization)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Event for feature action execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureActionEvent {
    pub feature: String,
    pub action: String,
    pub arguments: HashMap<String, Value>,
    pub request_id: Uuid,
}

impl Event for FeatureActionEvent {
    fn type_name() -> &'static str {
        "FeatureActionEvent"
    }

    fn serialize(&self) -> Result<Vec<u8>, EventError> {
        serde_json::to_vec(self).map_err(EventError::Serialization)
    }

    fn deserialize(data: &[u8]) -> Result<Self, EventError> {
        serde_json::from_slice(data).map_err(EventError::Serialization)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Event for feature action completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureActionCompleteEvent {
    pub request_id: Uuid,
    pub result: Result<Value, String>,
    pub execution_time_ms: u64,
}

impl Event for FeatureActionCompleteEvent {
    fn type_name() -> &'static str {
        "FeatureActionCompleteEvent"
    }

    fn serialize(&self) -> Result<Vec<u8>, EventError> {
        serde_json::to_vec(self).map_err(EventError::Serialization)
    }

    fn deserialize(data: &[u8]) -> Result<Self, EventError> {
        serde_json::from_slice(data).map_err(EventError::Serialization)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Event for argument registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentRegisteredEvent {
    pub plugin_name: String,
    pub argument_name: String,
    pub argument_type: String,
    pub is_global: bool,
}

impl Event for ArgumentRegisteredEvent {
    fn type_name() -> &'static str {
        "ArgumentRegisteredEvent"
    }

    fn serialize(&self) -> Result<Vec<u8>, EventError> {
        serde_json::to_vec(self).map_err(EventError::Serialization)
    }

    fn deserialize(data: &[u8]) -> Result<Self, EventError> {
        serde_json::from_slice(data).map_err(EventError::Serialization)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
//! # Event-Driven Plugin System
//!
//! This module provides a type-safe, event-driven plugin system that allows plugins
//! to register event handlers and declare their capabilities through features.
//!
//! ## Core Concepts
//!
//! - **Events**: Strongly-typed events that plugins can emit and handle
//! - **Features**: Declared capabilities like "VM_Manage", "File_Storage"
//! - **Dynamic Arguments**: Plugin-specific parameters managed centrally
//! - **No Case Statements**: All routing handled through event callbacks

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::RwLock as AsyncRwLock;
use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

pub mod events;
pub mod features;
pub mod plugin;
pub mod registry;
pub mod context;
pub mod arguments;
pub mod executor;

pub use events::*;
pub use features::*;
pub use plugin::*;
pub use plugin::Plugin; // Bring the Plugin trait into scope for method resolution
pub use registry::*;
pub use context::*;
pub use arguments::*;
pub use executor::*;

/// Main plugin system that manages events, plugins, and features
#[derive(Debug)]
pub struct PluginSystem {
    /// Event system for handling all plugin communication
    pub event_system: Arc<EventSystem>,
    /// Plugin registry for managing loaded plugins
    pub plugin_registry: Arc<PluginRegistry>,
    /// Feature registry for tracking declared capabilities
    pub feature_registry: Arc<FeatureRegistry>,
    /// Argument manager for handling dynamic parameters
    pub argument_manager: Arc<ArgumentManager>,
    /// Server context for plugin operations
    server_context: Arc<dyn ServerContext>,
}

impl PluginSystem {
    /// Create a new plugin system instance
    pub fn new(server_context: Arc<dyn ServerContext>) -> Self {
        let event_system = Arc::new(EventSystem::new());
        let plugin_registry = Arc::new(PluginRegistry::new());
        let feature_registry = Arc::new(FeatureRegistry::new());
        let argument_manager = Arc::new(ArgumentManager::new());

        Self {
            event_system,
            plugin_registry,
            feature_registry,
            argument_manager,
            server_context,
        }
    }

    /// Initialize the plugin system by loading feature schemas and plugins
    pub async fn initialize(&self) -> Result<(), PluginError> {
        // Load feature schemas from JSON files
        self.feature_registry.load_schemas("./features").await?;
        
        // Load plugins from the plugins directory
        self.plugin_registry.load_plugins("./plugins", Arc::clone(&self.event_system)).await?;
        
        // Initialize all loaded plugins
        for plugin_name in self.plugin_registry.list_plugins().await {
            self.initialize_plugin(&plugin_name).await?;
        }

        Ok(())
    }

    /// Initialize a specific plugin
    async fn initialize_plugin(&self, plugin_name: &str) -> Result<(), PluginError> {
        // Get plugin instance Arc
        let plugin_arc = self.plugin_registry.get_plugin(plugin_name).await
            .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

        // SAFETY: We must get a mutable reference to the plugin instance for initialization.
        // This is only safe if no other Arc clones exist. This mirrors the registry.rs logic.
        // Use the registry's initialize_plugin method to avoid accessing private fields
        self.plugin_registry.initialize_plugin(plugin_name, Arc::clone(&self.server_context)).await
    }

    /// Execute a feature action through the event system
    pub async fn execute_feature_action(
        &self,
        feature: &str,
        action: &str,
        args: HashMap<String, Value>,
    ) -> Result<Value, PluginError> {
        let event_key = format!("feature:{}:{}", feature, action);
        let event = FeatureActionEvent {
            feature: feature.to_string(),
            action: action.to_string(),
            arguments: args,
            request_id: Uuid::new_v4(),
        };

        self.event_system.emit_event(&event_key, &event).await
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // Wait for response (this would be implemented with proper async coordination)
        // For now, return a success indicator
        Ok(Value::Bool(true))
    }

    /// Get available features
    pub async fn get_available_features(&self) -> Vec<String> {
        self.feature_registry.list_features().await
    }

    /// Get available actions for a feature
    pub async fn get_feature_actions(&self, feature: &str) -> Result<Vec<String>, PluginError> {
        self.feature_registry.get_feature_actions(feature).await
    }

    /// Get required arguments for a feature action
    pub async fn get_action_arguments(&self, feature: &str, action: &str) -> Result<Vec<ArgumentDef>, PluginError> {
        self.feature_registry.get_action_arguments(feature, action).await
    }

    /// Shutdown the plugin system gracefully
    pub async fn shutdown(&self) -> Result<(), PluginError> {
        // Shutdown all plugins using the registry's shutdown_all, which handles Arc mutability correctly
        self.plugin_registry.shutdown_all(Arc::clone(&self.server_context)).await
    }
}

/// Errors that can occur in the plugin system
#[derive(Error, Debug, Clone)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
    
    #[error("Feature not supported: {0}")]
    UnsupportedFeature(String),
    
    #[error("Event system error: {0}")]
    EventError(String),
    
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    
    #[error("I/O error: {0}")]
    IoError(Arc<std::io::Error>),
    
    #[error("JSON error: {0}")]
    JsonError(Arc<serde_json::Error>),
}

impl From<std::io::Error> for PluginError {
    fn from(err: std::io::Error) -> Self {
        PluginError::IoError(Arc::new(err))
    }
}

impl From<serde_json::Error> for PluginError {
    fn from(err: serde_json::Error) -> Self {
        PluginError::JsonError(Arc::new(err))
    }
}

/// Convert plugin errors to event errors
impl From<PluginError> for EventError {
    fn from(error: PluginError) -> Self {
        EventError::HandlerExecution(error.to_string())
    }
}
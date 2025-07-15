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

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

pub mod events;
pub mod features;
pub mod plugin;
pub mod registry;
pub mod context;
pub mod arguments;
pub mod dynamic_api;
pub mod enum_features;
pub mod enhanced_executor;
pub mod event_executor;

pub use events::*;
pub use features::*;
pub use plugin::*;
pub use plugin::Plugin; // Bring the Plugin trait into scope for method resolution
pub use registry::*;
pub use context::{ServerContext, ServerContextBuilder, FeatureContext};
pub use arguments::*;
pub use dynamic_api::*;
pub use enum_features::*;
pub use enhanced_executor::*;
pub use event_executor::*;

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
    /// Enhanced feature manager for enum-based features
    pub enhanced_features: Arc<AsyncRwLock<EnhancedFeatureManager>>,
    /// Event-driven executor for direct command execution
    pub event_executor: Arc<EventDrivenExecutor>,
    /// Server context for plugin operations
    server_context: Arc<dyn ServerContext>,
}

impl PluginSystem {
    /// Create a new plugin system instance
    pub fn new(server_context: Arc<dyn ServerContext>) -> Self {
        // Use the event system from the provided server_context, not a new one
        let event_system = server_context.events();
        let plugin_registry = Arc::new(PluginRegistry::new());
        let feature_registry = Arc::new(FeatureRegistry::new());
        let argument_manager = Arc::new(ArgumentManager::new());
        let enhanced_features = Arc::new(AsyncRwLock::new(EnhancedFeatureManager::new()));
        let event_executor = Arc::new(EventDrivenExecutor::new());

        Self {
            event_system,
            plugin_registry,
            feature_registry,
            argument_manager,
            enhanced_features,
            event_executor,
            server_context,
        }
    }

    /// Initialize the plugin system by loading feature schemas and plugins
    pub async fn initialize(&self) -> Result<(), PluginError> {
        // Load feature schemas from JSON files
        self.feature_registry.load_schemas("./features").await?;

        // Initialize enhanced features
        {
            let mut enhanced_features = self.enhanced_features.write().await;
            enhanced_features.initialize().await?;
        }

        // Load plugins from the plugins directory, passing the main context
        self.plugin_registry.load_plugins(
            "./plugins",
            Arc::clone(&self.event_system),
            Arc::clone(&self.server_context),
        ).await?;

        // No need to call initialize_plugin again, as plugins are now initialized at load time
        Ok(())
    }

    /// Initialize a specific plugin
    async fn initialize_plugin(&self, plugin_name: &str) -> Result<(), PluginError> {
        // Get plugin instance Arc
        let _plugin_arc = self.plugin_registry.get_plugin(plugin_name).await
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

    /// Execute a command directly through the event-driven executor
    pub async fn execute_command(
        &self,
        provider: &str,
        feature: &str,
        method: &str,
        payload: Value,
    ) -> Result<Value, PluginError> {
        self.event_executor.execute_command(provider, feature, method, payload).await
    }

    /// List all available event handlers
    pub fn list_event_handlers(&self) -> Vec<String> {
        self.event_executor.list_available_handlers()
    }

    /// Get available features
    pub async fn get_available_features(&self) -> Vec<String> {
        let mut features = self.feature_registry.list_features().await;
        
        // Add enhanced features
        let enhanced_features = self.enhanced_features.read().await;
        features.extend(enhanced_features.get_available_features());
        
        features
    }

    /// Get available actions for a feature
    pub async fn get_feature_actions(&self, feature: &str) -> Result<Vec<String>, PluginError> {
        // First try enhanced features
        let enhanced_features = self.enhanced_features.read().await;
        if let Ok(actions) = enhanced_features.get_feature_operations(feature) {
            return Ok(actions);
        }
        
        // Fall back to legacy features
        self.feature_registry.get_feature_actions(feature).await
    }

    /// Get required arguments for a feature action
    pub async fn get_action_arguments(&self, feature: &str, action: &str) -> Result<Vec<ArgumentDef>, PluginError> {
        // First try enhanced features
        let enhanced_features = self.enhanced_features.read().await;
        if let Ok(args) = enhanced_features.get_operation_arguments(feature, action) {
            // Convert from Args to ArgumentDef
            let arg_defs: Vec<ArgumentDef> = args.into_iter().map(|(name, rust_type)| {
                ArgumentDef {
                    name,
                    description: format!("Parameter of type {}", rust_type),
                    arg_type: match rust_type.as_str() {
                        "String" => ArgumentType::String { max_length: None },
                        "i32" => ArgumentType::Number { min: None, max: None },
                        "bool" => ArgumentType::Boolean,
                        _ => ArgumentType::Any,
                    },
                    required: true,
                    default_value: None,
                    constraints: None,
                }
            }).collect();
            return Ok(arg_defs);
        }
        
        // Fall back to legacy features
        self.feature_registry.get_action_arguments(feature, action).await
    }

    /// Shutdown the plugin system gracefully
    pub async fn shutdown(&self) -> Result<(), PluginError> {
        // Shutdown all plugins using the registry's shutdown_all, which handles Arc mutability correctly
        self.plugin_registry.shutdown_all(Arc::clone(&self.server_context)).await
    }
}

/// Errors that can occur in the plugin system
#[derive(Error, Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    IoError(String),
    
    #[error("JSON error: {0}")]
    JsonError(String),
}

impl From<std::io::Error> for PluginError {
    fn from(err: std::io::Error) -> Self {
        PluginError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for PluginError {
    fn from(err: serde_json::Error) -> Self {
        PluginError::JsonError(err.to_string())
    }
}

/// Convert plugin errors to event errors
impl From<PluginError> for EventError {
    fn from(error: PluginError) -> Self {
        EventError::HandlerExecution(error.to_string())
    }
}
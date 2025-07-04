//! # Plugin System Core
//!
//! Defines the core plugin trait and lifecycle management.
//! Plugins implement this trait to register event handlers and declare features.

use std::sync::Arc;
use async_trait::async_trait;
use super::{PluginError, ServerContext};

/// Core plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin name (must be unique)
    fn name(&self) -> &str;
    
    /// Plugin version
    fn version(&self) -> &str;
    
    /// Features this plugin declares (e.g., ["VM_Manage", "File_Storage"])
    fn declared_features(&self) -> Vec<String>;
    
    /// Pre-initialization phase: register event handlers
    /// This is where plugins register their event handlers using the event system
    async fn pre_init(&mut self, context: Arc<dyn ServerContext>) -> Result<(), PluginError>;
    
    /// Initialization phase: setup and register arguments
    /// This is where plugins register their dynamic arguments and perform setup
    async fn init(&mut self, context: Arc<dyn ServerContext>) -> Result<(), PluginError>;
    
    /// Shutdown phase: cleanup resources
    async fn shutdown(&mut self, context: Arc<dyn ServerContext>) -> Result<(), PluginError>;
}

/// Plugin factory function type for dynamic loading
pub type PluginFactory = fn() -> Box<dyn Plugin>;

/// Macro to help create plugins with feature declarations
/// 
/// Usage:
/// ```rust
/// use crate::plugin_impl;
/// 
/// plugin_impl! {
///     MyCloudPlugin,
///     version: "1.0.0",
///     features: ["VM_Manage", "File_Storage"],
///     struct MyCloudPlugin {
///         config: MyConfig,
///     }
/// }
/// ```
#[macro_export]
macro_rules! plugin_impl {
    (
        $plugin_name:ident,
        version: $version:literal,
        features: [$($feature:literal),* $(,)?],
        struct $struct_name:ident {
            $($field:ident: $field_type:ty),* $(,)?
        }
    ) => {
        pub struct $struct_name {
            $($field: $field_type,)*
        }

        impl $struct_name {
            pub fn new($($field: $field_type),*) -> Self {
                Self {
                    $($field,)*
                }
            }
        }

        #[async_trait::async_trait]
        impl $crate::Plugin for $struct_name {
            fn name(&self) -> &str {
                stringify!($plugin_name)
            }

            fn version(&self) -> &str {
                $version
            }

            fn declared_features(&self) -> Vec<String> {
                vec![$($feature.to_string()),*]
            }

            async fn pre_init(&mut self, context: std::sync::Arc<dyn $crate::ServerContext>) -> Result<(), $crate::PluginError> {
                self.register_handlers(context).await
            }

            async fn init(&mut self, context: std::sync::Arc<dyn $crate::ServerContext>) -> Result<(), $crate::PluginError> {
                self.setup(context).await
            }

            async fn shutdown(&mut self, context: std::sync::Arc<dyn $crate::ServerContext>) -> Result<(), $crate::PluginError> {
                self.cleanup(context).await
            }
        }

        impl $struct_name {
            // These methods must be implemented by the plugin
            async fn register_handlers(&mut self, context: std::sync::Arc<dyn $crate::ServerContext>) -> Result<(), $crate::PluginError>;
            async fn setup(&mut self, context: std::sync::Arc<dyn $crate::ServerContext>) -> Result<(), $crate::PluginError>;
            async fn cleanup(&mut self, context: std::sync::Arc<dyn $crate::ServerContext>) -> Result<(), $crate::PluginError>;
        }

        // Export function for dynamic loading
        #[no_mangle]
        pub extern "C" fn create_plugin() -> *mut dyn $crate::Plugin {
            let plugin = Box::new($struct_name::new(/* default values would go here */));
            Box::into_raw(plugin)
        }
    };
}

/// Helper trait for plugin configuration
pub trait PluginConfig: Send + Sync {
    /// Load configuration from a file or environment
    fn load_config(&mut self) -> Result<(), PluginError>;
    
    /// Validate configuration
    fn validate_config(&self) -> Result<(), PluginError>;
}

/// Base plugin implementation that provides common functionality
#[derive(Debug)]
pub struct BasePlugin {
    name: String,
    version: String,
    features: Vec<String>,
}

impl BasePlugin {
    pub fn new(name: String, version: String, features: Vec<String>) -> Self {
        Self {
            name,
            version,
            features,
        }
    }
}

#[async_trait]
impl Plugin for BasePlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn declared_features(&self) -> Vec<String> {
        self.features.clone()
    }

    async fn pre_init(&mut self, _context: Arc<dyn ServerContext>) -> Result<(), PluginError> {
        // Default implementation does nothing
        Ok(())
    }

    async fn init(&mut self, _context: Arc<dyn ServerContext>) -> Result<(), PluginError> {
        // Default implementation does nothing
        Ok(())
    }

    async fn shutdown(&mut self, _context: Arc<dyn ServerContext>) -> Result<(), PluginError> {
        // Default implementation does nothing
        Ok(())
    }
}

/// Plugin metadata for registration
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub features: Vec<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<String>,
}

impl PluginMetadata {
    pub fn new(name: String, version: String, features: Vec<String>) -> Self {
        Self {
            name,
            version,
            features,
            description: None,
            author: None,
            license: None,
            dependencies: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_author(mut self, author: String) -> Self {
        self.author = Some(author);
        self
    }

    pub fn with_license(mut self, license: String) -> Self {
        self.license = Some(license);
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.dependencies = dependencies;
        self
    }
}

/// Plugin state for lifecycle management
#[derive(Debug, Clone, PartialEq)]
pub enum PluginState {
    Unloaded,
    Loading,
    PreInitialized,
    Initialized,
    Running,
    Stopping,
    Stopped,
    Failed(String),
}

/// Plugin instance wrapper for state management
pub struct PluginInstance {
    plugin: Box<dyn Plugin>,
    metadata: PluginMetadata,
    state: PluginState,
}

impl std::fmt::Debug for PluginInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginInstance")
            .field("metadata", &self.metadata)
            .field("state", &self.state)
            .finish()
    }
}

impl PluginInstance {
    pub fn new(plugin: Box<dyn Plugin>, metadata: PluginMetadata) -> Self {
        Self {
            plugin,
            metadata,
            state: PluginState::Unloaded,
        }
    }
    pub fn plugin(&self) -> &dyn Plugin {
        self.plugin.as_ref()
    }

    pub fn plugin_mut(&mut self) -> &mut dyn Plugin {
        self.plugin.as_mut()
    }

    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    pub fn state(&self) -> &PluginState {
        &self.state
    }

    pub fn set_state(&mut self, state: PluginState) {
        self.state = state;
    }

    /// Check if plugin is in a running state
    pub fn is_running(&self) -> bool {
        matches!(self.state, PluginState::Running)
    }

    /// Check if plugin is in a failed state
    pub fn is_failed(&self) -> bool {
        matches!(self.state, PluginState::Failed(_))
    }

    /// Get failure reason if plugin is failed
    pub fn failure_reason(&self) -> Option<&str> {
        match &self.state {
            PluginState::Failed(reason) => Some(reason),
            _ => None,
        }
    }
}
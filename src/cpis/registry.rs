//! # Plugin Registry
//!
//! Manages plugin loading, registration, and lifecycle.
//! Handles dynamic loading of plugins from shared libraries.

use super::{
    EventSystem, Plugin, PluginError, PluginInstance, PluginMetadata, PluginState,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::ffi::c_void;
use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::path::Path;

/// Registry for managing loaded plugins
#[derive(Debug)]
pub struct PluginRegistry {
    /// Map of plugin name to plugin instance
plugins: RwLock<HashMap<String, Arc<tokio::sync::RwLock<PluginInstance>>>>,
    /// Loaded libraries to keep them in memory
    libraries: RwLock<Vec<Library>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            libraries: RwLock::new(Vec::new()),
        }
    }

    /// Load plugins from a directory
    pub async fn load_plugins<P: AsRef<Path>>(
        &self,
        plugins_dir: P,
        event_system: Arc<EventSystem>,
        context: Arc<dyn super::ServerContext>,
    ) -> Result<usize, PluginError> {
        let plugins_dir = plugins_dir.as_ref();

        if !plugins_dir.exists() {
            tokio::fs::create_dir_all(plugins_dir).await?;
            return Ok(0);
        }

        let mut loaded_count = 0;
        let mut read_dir = tokio::fs::read_dir(plugins_dir).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();

            // Check for plugin libraries
            #[cfg(target_os = "windows")]
            let is_plugin_lib = path.extension().and_then(|s| s.to_str()) == Some("dll");

            #[cfg(target_os = "linux")]
            let is_plugin_lib = path.extension().and_then(|s| s.to_str()) == Some("so");

            #[cfg(target_os = "macos")]
            let is_plugin_lib = path.extension().and_then(|s| s.to_str()) == Some("dylib");

            if is_plugin_lib {
                match self.load_plugin_from_library(&path, Arc::clone(&context)).await {
                    Ok(_) => loaded_count += 1,
                    Err(e) => eprintln!("Failed to load plugin from {:?}: {}", path, e),
                }
            }
        }

        Ok(loaded_count)
    }

    /// Load a plugin from a shared library
    async fn load_plugin_from_library<P: AsRef<Path>>(
        &self,
        library_path: P,
        context: Arc<dyn super::ServerContext>,
    ) -> Result<(), PluginError> {
        let library_path = library_path.as_ref();

        println!("Loading CPI from file: {:?}", library_path);

        // Load the library
        let lib = unsafe {
            Library::new(library_path).map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to load library {:?}: {}",
                    library_path, e
                ))
            })?
        };

        println!("Successfully loaded library: {:?}", library_path);

        // Get the plugin factory function (returns *mut dyn Plugin)
        let create_plugin: Symbol<unsafe extern "C" fn() -> *mut dyn Plugin> = unsafe {
            lib.get(b"create_plugin").map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to find create_plugin function: {}",
                    e
                ))
            })?
        };

        println!("Found create_plugin function in library: {:?}", library_path);

        println!("Creating plugin instance...");

        // Create the plugin instance
        let raw_ptr = unsafe { create_plugin() };
        if raw_ptr.is_null() {
            return Err(PluginError::InitializationFailed("create_plugin returned null pointer".to_string()));
        }
        // SAFETY: The plugin must be implemented as Box<dyn Plugin> in the plugin crate
        let plugin: Box<dyn Plugin> = unsafe { Box::from_raw(raw_ptr) };

        let plugin_name = plugin.name().to_string();
        let plugin_version = plugin.version().to_string();
        let plugin_features = plugin.declared_features();

        println!(
            "Plugin created: {} (version: {}, features: {:?})",
            plugin_name, plugin_version, plugin_features
        );
        // Create metadata
        let metadata = PluginMetadata::new(plugin_name.clone(), plugin_version, plugin_features);

        // Create plugin instance
        let mut plugin_instance = PluginInstance::new(plugin, metadata);
        plugin_instance.set_state(PluginState::Loading);

        // Call pre_init and init with the correct context before storing
        plugin_instance.set_state(PluginState::PreInitialized);
        plugin_instance.plugin_mut().pre_init(Arc::clone(&context)).await?;
        plugin_instance.set_state(PluginState::Initialized);
        plugin_instance.plugin_mut().init(Arc::clone(&context)).await?;
        plugin_instance.set_state(PluginState::Running);

        // Store the library and plugin
        {
            let mut libraries = self.libraries.write().await;
            libraries.push(lib);
        }

        {
            let mut plugins = self.plugins.write().await;
            if plugins.contains_key(&plugin_name) {
                return Err(PluginError::InitializationFailed(format!(
                    "Plugin with name '{}' already loaded",
                    plugin_name
                )));
            }
            plugins.insert(plugin_name.clone(), Arc::new(tokio::sync::RwLock::new(plugin_instance)));
        }

        println!("Loaded plugin: {} from {:?}", plugin_name, library_path);
        Ok(())
    }

    /// Register a plugin directly (for in-process plugins)
    pub async fn register_plugin(&self, plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let plugin_name = plugin.name().to_string();
        let plugin_version = plugin.version().to_string();
        let plugin_features = plugin.declared_features();

        // Create metadata
        let metadata = PluginMetadata::new(plugin_name.clone(), plugin_version, plugin_features);

        // Create plugin instance
        let plugin_instance = PluginInstance::new(plugin, metadata);

        // Store the plugin
        let mut plugins = self.plugins.write().await;
        if plugins.contains_key(&plugin_name) {
            return Err(PluginError::InitializationFailed(format!(
                "Plugin with name '{}' already registered",
                plugin_name
            )));
        }
        plugins.insert(plugin_name.clone(), Arc::new(tokio::sync::RwLock::new(plugin_instance)));

        println!("Registered plugin: {}", plugin_name);
        Ok(())
    }

    /// Get a plugin by name
    pub async fn get_plugin(&self, name: &str) -> Option<Arc<tokio::sync::RwLock<PluginInstance>>> {
        let plugins = self.plugins.read().await;
        plugins.get(name).cloned()
    }

    /// Get plugin metadata by name
    pub async fn get_plugin_metadata(&self, name: &str) -> Option<PluginMetadata> {
        let plugins = self.plugins.read().await;
        if let Some(instance) = plugins.get(name) {
            let instance = instance.read().await;
            Some(instance.metadata().clone())
        } else {
            None
        }
    }

    /// List all loaded plugin names
    pub async fn list_plugins(&self) -> Vec<String> {
        let plugins = self.plugins.read().await;
        plugins.keys().cloned().collect()
    }

    /// Get plugin state
    pub async fn get_plugin_state(&self, name: &str) -> Option<PluginState> {
        let plugins = self.plugins.read().await;
        if let Some(instance) = plugins.get(name) {
            let instance = instance.read().await;
            Some(instance.state().clone())
        } else {
            None
        }
    }

    /// Set plugin state
    pub async fn set_plugin_state(
        &self,
        name: &str,
        state: PluginState,
    ) -> Result<(), PluginError> {
        let plugins = self.plugins.read().await;
        let instance = plugins
            .get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;
        let mut instance_mut = instance.write().await;
        instance_mut.set_state(state);
        Ok(())
    }

    /// Initialize a plugin
    pub async fn initialize_plugin(
        &self,
        name: &str,
        context: Arc<dyn super::ServerContext>,
    ) -> Result<(), PluginError> {
        // Pre-init and Init phase
        let plugins = self.plugins.read().await;
        let instance = plugins
            .get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;
        let mut instance_mut = instance.write().await;
        instance_mut.set_state(PluginState::PreInitialized);
        instance_mut.plugin_mut().pre_init(Arc::clone(&context)).await?;
        instance_mut.set_state(PluginState::Initialized);
        instance_mut.plugin_mut().init(Arc::clone(&context)).await?;
        instance_mut.set_state(PluginState::Running);
        Ok(())
    }

    /// Shutdown a plugin
    pub async fn shutdown_plugin(
        &self,
        name: &str,
        context: Arc<dyn super::ServerContext>,
    ) -> Result<(), PluginError> {
        let plugins = self.plugins.read().await;
        let instance = plugins
            .get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;

        let mut instance_mut = instance.write().await;
        instance_mut.set_state(PluginState::Stopping);

        match instance_mut.plugin_mut().shutdown(context).await {
            Ok(_) => {
                instance_mut.set_state(PluginState::Stopped);
                Ok(())
            }
            Err(e) => {
                instance_mut.set_state(PluginState::Failed(e.to_string()));
                Err(e)
            }
        }
    }

    /// Remove a plugin from the registry
    pub async fn unload_plugin(&self, name: &str) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;

        if let Some(instance) = plugins.remove(name) {
            let instance = instance.read().await;
            // Ensure plugin is stopped
            if instance.is_running() {
                return Err(PluginError::ExecutionFailed(format!(
                    "Cannot unload running plugin: {}",
                    name
                )));
            }

            println!("Unloaded plugin: {}", name);
            Ok(())
        } else {
            Err(PluginError::PluginNotFound(name.to_string()))
        }
    }

    /// Get plugins that support a specific feature
    pub async fn get_plugins_by_feature(&self, feature: &str) -> Vec<String> {
        let plugins = self.plugins.read().await;
        let mut result = Vec::new();
        for (name, instance) in plugins.iter() {
            let instance = instance.read().await;
            if instance.metadata().features.contains(&feature.to_string()) {
                result.push(name.clone());
            }
        }
        result
    }

    /// Get all plugin statistics
    pub async fn get_plugin_stats(&self) -> PluginRegistryStats {
        let plugins = self.plugins.read().await;
        let total_plugins = plugins.len();

        let mut running_plugins = 0;
        let mut failed_plugins = 0;
        let mut stopped_plugins = 0;

        for instance in plugins.values() {
            let instance = instance.read().await;
            match instance.state() {
                PluginState::Running => running_plugins += 1,
                PluginState::Failed(_) => failed_plugins += 1,
                PluginState::Stopped => stopped_plugins += 1,
                _ => {}
            }
        }

        PluginRegistryStats {
            total_plugins,
            running_plugins,
            failed_plugins,
            stopped_plugins,
        }
    }

    /// Shutdown all plugins gracefully
    pub async fn shutdown_all(
        &self,
        context: Arc<dyn super::ServerContext>,
    ) -> Result<(), PluginError> {
        let plugin_names = self.list_plugins().await;

        for plugin_name in plugin_names {
            if let Err(e) = self
                .shutdown_plugin(&plugin_name, Arc::clone(&context))
                .await
            {
                eprintln!("Failed to shutdown plugin {}: {}", plugin_name, e);
            }
        }

        Ok(())
    }
}

/// Plugin registry statistics
#[derive(Debug, Clone)]
pub struct PluginRegistryStats {
    pub total_plugins: usize,
    pub running_plugins: usize,
    pub failed_plugins: usize,
    pub stopped_plugins: usize,
}

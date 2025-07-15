//! # Plugin Registry
//!
//! Manages plugin loading, registration, and lifecycle.
//! Handles dynamic loading of plugins from shared libraries.

use super::{
    EventSystem, Plugin, PluginError, PluginInstance, PluginMetadata, PluginState,
};
use omni_event_registry::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::path::Path;

// Plugin wrapper structure from the macros
#[repr(C)]
struct PluginWrapper {
    _data: [u8; 0],
}

/// Event-driven plugin implementation that dispatches to the global registry
struct EventDrivenPlugin {
    name: String,
    version: String,
    declared_features: Vec<String>,
    wrapper_ptr: *mut PluginWrapper,
}

#[async_trait::async_trait]
impl Plugin for EventDrivenPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn version(&self) -> &str {
        &self.version
    }
    
    fn declared_features(&self) -> Vec<String> {
        self.declared_features.clone()
    }
    
    async fn pre_init(&mut self, _context: Arc<dyn super::ServerContext>) -> Result<(), PluginError> {
        println!("🔧 Pre-initializing event-driven plugin: {}", self.name);
        Ok(())
    }
    
    async fn init(&mut self, _context: Arc<dyn super::ServerContext>) -> Result<(), PluginError> {
        println!("✅ Initialized event-driven plugin: {}", self.name);
        println!("📋 Registered handlers in global event registry");
        
        // List registered handlers
        let handlers = get_global_registry().list_handlers();
        for handler in &handlers {
            println!("   • {}", handler);
        }
        
        Ok(())
    }
    
    async fn shutdown(&mut self, _context: Arc<dyn super::ServerContext>) -> Result<(), PluginError> {
        println!("🛑 Shutting down event-driven plugin: {}", self.name);
        Ok(())
    }
}

unsafe impl Send for EventDrivenPlugin {}
unsafe impl Sync for EventDrivenPlugin {}

/// Feature plugin implementation that wraps the feature interface
struct FeaturePlugin {
    name: String,
    version: String,
    declared_features: Vec<String>,
    operations: Vec<String>,
    library_path: String,
}

#[async_trait::async_trait]
impl Plugin for FeaturePlugin {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn version(&self) -> &str {
        &self.version
    }
    
    fn declared_features(&self) -> Vec<String> {
        self.declared_features.clone()
    }
    
    async fn pre_init(&mut self, _context: Arc<dyn super::ServerContext>) -> Result<(), PluginError> {
        println!("🔧 Pre-initializing feature plugin: {}", self.name);
        Ok(())
    }
    
    async fn init(&mut self, _context: Arc<dyn super::ServerContext>) -> Result<(), PluginError> {
        println!("✅ Initialized feature plugin: {}", self.name);
        println!("📋 Available operations: {:?}", self.operations);
        Ok(())
    }
    
    async fn shutdown(&mut self, _context: Arc<dyn super::ServerContext>) -> Result<(), PluginError> {
        println!("🛑 Shutting down feature plugin: {}", self.name);
        Ok(())
    }
}

unsafe impl Send for FeaturePlugin {}
unsafe impl Sync for FeaturePlugin {}

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
        _event_system: Arc<EventSystem>,
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

        println!("Loading plugin from file: {:?}", library_path);

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

        // Try to determine if this is a CPI or a Feature based on available functions
        let is_cpi = unsafe { lib.get::<Symbol<unsafe extern "C" fn() -> *mut PluginWrapper>>(b"create_plugin").is_ok() };
        let is_feature = unsafe { lib.get::<Symbol<unsafe extern "C" fn() -> *const std::os::raw::c_char>>(b"get_feature_name").is_ok() };

        if is_cpi {
            println!("Detected CPI plugin");
            return self.load_cpi_plugin(lib, library_path, context).await;
        } else if is_feature {
            println!("Detected Feature plugin");
            return self.load_feature_plugin(lib, library_path, context).await;
        } else {
            return Err(PluginError::InitializationFailed(
                "Library does not contain recognized plugin interface (neither CPI nor Feature)".to_string()
            ));
        }
    }

    /// Load a CPI plugin
    async fn load_cpi_plugin<P: AsRef<Path>>(
        &self,
        lib: Library,
        library_path: P,
        context: Arc<dyn super::ServerContext>,
    ) -> Result<(), PluginError> {
        let library_path = library_path.as_ref();

        // Try to register handlers with the global event system (optional)
        let register_handlers_result: Result<Symbol<unsafe extern "C" fn()>, _> = unsafe {
            lib.get(b"register_handlers")
        };

        match register_handlers_result {
            Ok(register_handlers) => {
                println!("Found register_handlers function, registering handlers...");
                unsafe { register_handlers() };
            }
            Err(_) => {
                println!("No register_handlers function found, skipping handler registration...");
            }
        }

        // Get the plugin factory function (returns *mut PluginWrapper)
        let create_plugin: Symbol<unsafe extern "C" fn() -> *mut PluginWrapper> = unsafe {
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
        let wrapper_ptr = unsafe { create_plugin() };
        if wrapper_ptr.is_null() {
            return Err(PluginError::InitializationFailed("create_plugin returned null pointer".to_string()));
        }

        // Get plugin info via FFI
        let get_plugin_name: Symbol<unsafe extern "C" fn(*mut PluginWrapper) -> *const std::os::raw::c_char> = unsafe {
            lib.get(b"get_plugin_name").map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to find get_plugin_name function: {}",
                    e
                ))
            })?
        };

        let name_ptr = unsafe { get_plugin_name(wrapper_ptr) };
        let plugin_name = if !name_ptr.is_null() {
            unsafe { 
                std::ffi::CStr::from_ptr(name_ptr)
                    .to_str()
                    .unwrap_or("Unknown Plugin")
                    .to_string()
            }
        } else {
            "Unknown Plugin".to_string()
        };

        // For now, create a dummy plugin implementation that uses the event system
        let plugin: Box<dyn Plugin> = Box::new(EventDrivenPlugin {
            name: plugin_name.clone(),
            version: "1.0.0".to_string(),
            declared_features: vec!["VmManagement".to_string(), "VmControl".to_string(), "VmMonitoring".to_string()],
            wrapper_ptr,
        });

        let plugin_name = plugin.name().to_string();
        let plugin_version = plugin.version().to_string();
        let plugin_features = plugin.declared_features();

        println!(
            "Plugin created: {} (version: {}, features: {:?})",
            plugin_name, plugin_version, plugin_features
        );
        
        // Create metadata with file path
        let metadata = PluginMetadata::new(plugin_name.clone(), plugin_version, plugin_features)
            .with_file_path(library_path.to_string_lossy().to_string());

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

    /// Load a Feature plugin
    async fn load_feature_plugin<P: AsRef<Path>>(
        &self,
        lib: Library,
        library_path: P,
        context: Arc<dyn super::ServerContext>,
    ) -> Result<(), PluginError> {
        let library_path = library_path.as_ref();

        // Get feature name
        let get_feature_name: Symbol<unsafe extern "C" fn() -> *const std::os::raw::c_char> = unsafe {
            lib.get(b"get_feature_name").map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to find get_feature_name function: {}",
                    e
                ))
            })?
        };

        let name_ptr = unsafe { get_feature_name() };
        let feature_name = if !name_ptr.is_null() {
            unsafe { 
                std::ffi::CStr::from_ptr(name_ptr)
                    .to_str()
                    .unwrap_or("Unknown Feature")
                    .to_string()
            }
        } else {
            "Unknown Feature".to_string()
        };

        // Get operations
        let register_operations: Symbol<unsafe extern "C" fn() -> *const std::os::raw::c_char> = unsafe {
            lib.get(b"register_operations").map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to find register_operations function: {}",
                    e
                ))
            })?
        };

        let operations_ptr = unsafe { register_operations() };
        let operations_json = if !operations_ptr.is_null() {
            unsafe { 
                std::ffi::CStr::from_ptr(operations_ptr)
                    .to_str()
                    .unwrap_or("[]")
                    .to_string()
            }
        } else {
            "[]".to_string()
        };

        let operations: Vec<String> = serde_json::from_str(&operations_json).unwrap_or_else(|_| vec![]);

        println!("Feature loaded: {} with operations: {:?}", feature_name, operations);

        // Create a feature-based plugin that wraps the feature interface
        let plugin: Box<dyn Plugin> = Box::new(FeaturePlugin {
            name: feature_name.clone(),
            version: "1.0.0".to_string(),
            declared_features: vec![feature_name.clone()],
            operations,
            library_path: library_path.to_string_lossy().to_string(),
        });

        let plugin_name = plugin.name().to_string();
        let plugin_version = plugin.version().to_string();
        let plugin_features = plugin.declared_features();

        println!(
            "Feature plugin created: {} (version: {}, features: {:?})",
            plugin_name, plugin_version, plugin_features
        );
        
        // Create metadata with file path
        let metadata = PluginMetadata::new(plugin_name.clone(), plugin_version, plugin_features)
            .with_file_path(library_path.to_string_lossy().to_string());

        // Create plugin instance
        let mut plugin_instance = PluginInstance::new(plugin, metadata);
        plugin_instance.set_state(PluginState::Loading);

        // Call pre_init and init
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
                    "Feature plugin with name '{}' already loaded",
                    plugin_name
                )));
            }
            plugins.insert(plugin_name.clone(), Arc::new(tokio::sync::RwLock::new(plugin_instance)));
        }

        println!("Loaded feature plugin: {} from {:?}", plugin_name, library_path);
        Ok(())
    }

    /// Register a plugin directly (for in-process plugins)
    pub async fn register_plugin(&self, plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let plugin_name = plugin.name().to_string();
        let plugin_version = plugin.version().to_string();
        let plugin_features = plugin.declared_features();

        // Create metadata (in-process plugins don't have a file path)
        let metadata = PluginMetadata::new(plugin_name.clone(), plugin_version, plugin_features)
            .with_file_path("<in-process>".to_string());

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

//! # Provider Loader
//!
//! Handles dynamic loading of providers from shared libraries.

use super::{Provider, ProviderError, ProviderResult, ProviderMetadata, FeatureMetadata, ProviderContext, FeatureInterface, FeatureOperation, EventRegistry};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use libloading::{Library, Symbol};
use tokio::sync::RwLock;

/// Provider loader manages dynamic loading of provider libraries
pub struct ProviderLoader {
    /// Loaded libraries (kept alive)
    libraries: RwLock<Vec<Library>>,
}

/// Provider factory function signature
type ProviderFactoryFn = unsafe extern "C" fn() -> *mut std::ffi::c_void;

/// Provider metadata function signature  
type ProviderMetadataFn = unsafe extern "C" fn() -> *const std::os::raw::c_char;

impl ProviderLoader {
    /// Create a new provider loader
    pub fn new() -> Self {
        Self {
            libraries: RwLock::new(Vec::new()),
        }
    }
    
    /// Load providers from a directory
    pub async fn load_from_directory<P: AsRef<Path>>(
        &self,
        directory: P,
        context: Arc<dyn ProviderContext>,
    ) -> ProviderResult<Vec<(Arc<dyn Provider>, ProviderMetadata)>> {
        self.load_from_directory_with_registry(directory, context, None).await
    }
    
    /// Load providers from a directory with optional event registry
    pub async fn load_from_directory_with_registry<P: AsRef<Path>>(
        &self,
        directory: P,
        context: Arc<dyn ProviderContext>,
        event_registry: Option<Arc<EventRegistry>>,
    ) -> ProviderResult<Vec<(Arc<dyn Provider>, ProviderMetadata)>> {
        let directory = directory.as_ref();
        
        if !directory.exists() {
            tokio::fs::create_dir_all(directory).await?;
            return Ok(Vec::new());
        }
        
        let mut providers = Vec::new();
        let mut read_dir = tokio::fs::read_dir(directory).await?;
        
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            
            // Check for provider libraries
            if self.is_provider_library(&path) {
                match self.load_provider_from_file_with_registry(&path, Arc::clone(&context), event_registry.clone()).await {
                    Ok((provider, metadata)) => {
                        providers.push((provider, metadata));
                    }
                    Err(e) => {
                        eprintln!("Failed to load provider from {:?}: {}", path, e);
                    }
                }
            }
        }
        
        Ok(providers)
    }
    
    /// Load feature interfaces from a directory (does not register as callable providers)
    pub async fn load_features_from_directory<P: AsRef<Path>>(
        &self,
        directory: P,
    ) -> ProviderResult<Vec<FeatureInterface>> {
        let directory = directory.as_ref();
        
        if !directory.exists() {
            tokio::fs::create_dir_all(directory).await?;
            return Ok(Vec::new());
        }
        
        let mut features = Vec::new();
        let mut read_dir = tokio::fs::read_dir(directory).await?;
        
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            
            // Check for feature libraries
            if self.is_provider_library(&path) {
                match self.load_feature_interface_from_file(&path).await {
                    Ok(feature) => {
                        features.push(feature);
                    }
                    Err(e) => {
                        eprintln!("Failed to load feature interface from {:?}: {}", path, e);
                    }
                }
            }
        }
        
        Ok(features)
    }
    
    /// Check if a file is a provider library
    fn is_provider_library(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
            match extension {
                #[cfg(target_os = "windows")]
                "dll" => true,
                #[cfg(target_os = "linux")]
                "so" => true,
                #[cfg(target_os = "macos")]
                "dylib" => true,
                _ => false,
            }
        } else {
            false
        }
    }
    
    /// Load a provider from a specific file
    pub async fn load_provider_from_file<P: AsRef<Path>>(
        &self,
        library_path: P,
        context: Arc<dyn ProviderContext>,
    ) -> ProviderResult<(Arc<dyn Provider>, ProviderMetadata)> {
        self.load_provider_from_file_with_registry(library_path, context, None).await
    }
    
    /// Load a provider from a specific file with optional event registry
    pub async fn load_provider_from_file_with_registry<P: AsRef<Path>>(
        &self,
        library_path: P,
        context: Arc<dyn ProviderContext>,
        event_registry: Option<Arc<EventRegistry>>,
    ) -> ProviderResult<(Arc<dyn Provider>, ProviderMetadata)> {
        let library_path = library_path.as_ref();
        
        println!("Loading provider from file: {:?}", library_path);
        
        // Load the library
        let lib = unsafe {
            Library::new(library_path).map_err(|e| {
                ProviderError::LoadingFailed(format!(
                    "Failed to load library {:?}: {}",
                    library_path, e
                ))
            })?
        };
        
        println!("Successfully loaded library: {:?}", library_path);
        
        // Determine if this is a legacy CPI or new-style provider
        let provider_type = self.detect_provider_type(&lib)?;
        
        match provider_type {
            ProviderType::LegacyCpi => {
                self.load_legacy_cpi(&lib, library_path, context, event_registry).await
            }
            ProviderType::ModernProvider => {
                self.load_modern_provider(&lib, library_path, context, event_registry).await
            }
            ProviderType::Feature => {
                // Features are interface definitions, not callable providers
                Err(ProviderError::LoadingFailed(
                    "Feature interfaces should be loaded separately, not as providers".to_string()
                ))
            }
        }
    }
    
    /// Detect what type of provider this library contains
    fn detect_provider_type(&self, lib: &Library) -> ProviderResult<ProviderType> {
        // Check for modern provider interface
        if unsafe { lib.get::<Symbol<ProviderFactoryFn>>(b"create_provider").is_ok() } {
            return Ok(ProviderType::ModernProvider);
        }
        
        // Check for legacy CPI interface  
        if unsafe { lib.get::<Symbol<ProviderFactoryFn>>(b"create_plugin").is_ok() } {
            return Ok(ProviderType::LegacyCpi);
        }
        
        // Check for feature interface
        if unsafe { lib.get::<Symbol<ProviderMetadataFn>>(b"get_feature_name").is_ok() } {
            return Ok(ProviderType::Feature);
        }
        
        Err(ProviderError::LoadingFailed(
            "Library does not contain recognized provider interface".to_string()
        ))
    }
    
    /// Load a feature interface from a specific file
    pub async fn load_feature_interface_from_file<P: AsRef<Path>>(
        &self,
        library_path: P,
    ) -> ProviderResult<FeatureInterface> {
        let library_path = library_path.as_ref();
        
        println!("Loading feature interface from file: {:?}", library_path);
        
        // Load the library
        let lib = unsafe {
            Library::new(library_path).map_err(|e| {
                ProviderError::LoadingFailed(format!(
                    "Failed to load library {:?}: {}",
                    library_path, e
                ))
            })?
        };
        
        // Verify it's a feature interface
        let provider_type = self.detect_provider_type(&lib)?;
        if !matches!(provider_type, ProviderType::Feature) {
            return Err(ProviderError::LoadingFailed(
                "Library is not a feature interface".to_string()
            ));
        }
        
        // Get feature name
        let get_feature_name: Symbol<unsafe extern "C" fn() -> *const std::os::raw::c_char> = unsafe {
            lib.get(b"get_feature_name").map_err(|e| {
                ProviderError::LoadingFailed(format!(
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
                    .unwrap_or("unknown")
                    .to_string()
            }
        } else {
            "unknown".to_string()
        };
        
        // For now, create a minimal feature interface
        // In a full implementation, you'd extract operations from the library
        let feature_interface = FeatureInterface {
            name: feature_name,
            description: format!("Feature interface loaded from {:?}", library_path),
            operations: vec![
                // This would be dynamically extracted from the library
                FeatureOperation {
                    name: "example_operation".to_string(),
                    description: "Example operation".to_string(),
                    parameters: HashMap::new(),
                    returns: Some("Value".to_string()),
                }
            ],
        };
        
        println!("Successfully loaded feature interface: {}", feature_interface.name);
        
        Ok(feature_interface)
    }
    
    /// Load a legacy CPI and wrap it as a provider
    async fn load_legacy_cpi(
        &self,
        lib: &Library,
        library_path: &Path,
        context: Arc<dyn ProviderContext>,
        event_registry: Option<Arc<EventRegistry>>,
    ) -> ProviderResult<(Arc<dyn Provider>, ProviderMetadata)> {
        println!("Detected legacy CPI plugin");
        
        // For now, create a wrapper that adapts the CPI to our provider interface
        let provider = Arc::new(LegacyCpiAdapter::new(
            library_path.to_string_lossy().to_string()
        ));
        
        let metadata = ProviderMetadata {
            name: "legacy-cpi".to_string(),
            version: "1.0.0".to_string(),
            description: "Legacy CPI adapter".to_string(),
            author: None,
            license: None,
            features: vec![], // Features are loaded separately
            settings_schema: None,
            file_path: Some(library_path.to_string_lossy().to_string()),
            metadata: None, // Legacy CPIs don't have raw metadata
        };
        
        // Store the library to keep it alive
        {
            let mut libraries = self.libraries.write().await;
            // We can't move lib here because we borrowed it, so we need to reload it
            let owned_lib = unsafe { Library::new(library_path)? };
            libraries.push(owned_lib);
        }
        
        Ok((provider, metadata))
    }
    
    /// Load a modern provider
    async fn load_modern_provider(
        &self,
        lib: &Library,
        library_path: &Path,
        context: Arc<dyn ProviderContext>,
        event_registry: Option<Arc<EventRegistry>>,
    ) -> ProviderResult<(Arc<dyn Provider>, ProviderMetadata)> {
        println!("Detected modern provider");
        
        // Get the create_provider function
        let create_provider: Symbol<unsafe extern "C" fn() -> *mut std::ffi::c_void> = unsafe {
            lib.get(b"create_provider").map_err(|e| {
                ProviderError::LoadingFailed(format!(
                    "Failed to find create_provider function: {}",
                    e
                ))
            })?
        };
        
        // Get the metadata function
        let get_metadata: Symbol<unsafe extern "C" fn() -> *const std::os::raw::c_char> = unsafe {
            lib.get(b"get_provider_metadata").map_err(|e| {
                ProviderError::LoadingFailed(format!(
                    "Failed to find get_provider_metadata function: {}",
                    e
                ))
            })?
        };
        
        // Get the initialize function
        let initialize_provider: Symbol<unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> bool> = unsafe {
            lib.get(b"initialize_provider").map_err(|e| {
                ProviderError::LoadingFailed(format!(
                    "Failed to find initialize_provider function: {}",
                    e
                ))
            })?
        };
        
        // Create the provider instance
        let provider_ptr = unsafe { create_provider() };
        if provider_ptr.is_null() {
            return Err(ProviderError::LoadingFailed(
                "create_provider returned null".to_string()
            ));
        }
        
        // Get metadata
        let metadata_ptr = unsafe { get_metadata() };
        let metadata_str = if !metadata_ptr.is_null() {
            unsafe { std::ffi::CStr::from_ptr(metadata_ptr).to_str().unwrap_or("{}") }
        } else {
            "{}"
        };
        
        let metadata_json: serde_json::Value = serde_json::from_str(metadata_str)
            .map_err(|e| ProviderError::LoadingFailed(format!("Invalid metadata JSON: {}", e)))?;
        
        // Initialize the provider with event registry if available
        if let Some(registry) = event_registry {
            // Create a registry adapter that bridges the FFI interface to our event registry
            let mut registry_adapter = Box::new(RegistryAdapter::new());
            registry_adapter.set_registry(registry);
            let registry_ptr = Box::into_raw(registry_adapter) as *mut std::ffi::c_void;
            
            let init_success = unsafe { initialize_provider(provider_ptr, registry_ptr) };
            if !init_success {
                return Err(ProviderError::InitializationFailed(
                    "Provider initialization failed".to_string()
                ));
            }
        } else {
            // No event registry provided, skip initialization
            println!("Warning: No event registry provided for modern provider initialization");
        }
        
        // Create the provider wrapper
        let provider = Arc::new(ModernProviderWrapper::new(
            provider_ptr,
            metadata_json.clone(),
            library_path.to_path_buf(),
        ));
        
        // Create metadata - modern providers don't expose features
        let metadata = ProviderMetadata {
            name: metadata_json.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            version: metadata_json.get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0")
                .to_string(),
            description: metadata_json.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            author: None,
            license: None,
            features: vec![], // Features are loaded separately
            settings_schema: None,
            file_path: Some(library_path.to_string_lossy().to_string()),
            metadata: Some(metadata_json), // Store raw metadata for validation
        };
        
        // Store the library to keep it alive
        {
            let mut libraries = self.libraries.write().await;
            let owned_lib = unsafe { Library::new(library_path)? };
            libraries.push(owned_lib);
        }
        
        Ok((provider, metadata))
    }
    
    /// Load a feature and wrap it as a provider
    async fn load_feature_as_provider(
        &self,
        lib: &Library,
        library_path: &Path,
        context: Arc<dyn ProviderContext>,
    ) -> ProviderResult<(Arc<dyn Provider>, ProviderMetadata)> {
        println!("Detected feature plugin");
        
        // Get feature name
        let get_feature_name: Symbol<unsafe extern "C" fn() -> *const std::os::raw::c_char> = unsafe {
            lib.get(b"get_feature_name").map_err(|e| {
                ProviderError::LoadingFailed(format!(
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
                    .unwrap_or("unknown")
                    .to_string()
            }
        } else {
            "unknown".to_string()
        };
        
        // Get operations
        let register_operations: Symbol<unsafe extern "C" fn() -> *const std::os::raw::c_char> = unsafe {
            lib.get(b"register_operations").map_err(|e| {
                ProviderError::LoadingFailed(format!(
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
        
        let operations: Vec<String> = serde_json::from_str(&operations_json)
            .unwrap_or_else(|_| vec![]);
        
        println!("Feature loaded: {} with operations: {:?}", feature_name, operations);
        
        // Create feature adapter
        let provider = Arc::new(FeatureAdapter::new(
            feature_name.clone(),
            operations.clone(),
            library_path.to_string_lossy().to_string(),
        ));
        
        // Build metadata
        let feature_metadata = FeatureMetadata::new(
            feature_name.clone(),
            format!("{} feature", feature_name),
            "1.0.0".to_string(),
        );
        
        let metadata = ProviderMetadata {
            name: format!("feature-{}", feature_name),
            version: "1.0.0".to_string(),
            description: format!("Feature provider for {}", feature_name),
            author: None,
            license: None,
            features: vec![feature_metadata],
            settings_schema: None,
            file_path: Some(library_path.to_string_lossy().to_string()),
            metadata: None, // Feature adapters don't have raw metadata
        };
        
        // Store the library to keep it alive
        {
            let mut libraries = self.libraries.write().await;
            let owned_lib = unsafe { Library::new(library_path)? };
            libraries.push(owned_lib);
        }
        
        Ok((provider, metadata))
    }
}

/// Types of providers that can be loaded
#[derive(Debug, Clone, Copy)]
enum ProviderType {
    LegacyCpi,
    ModernProvider,
    Feature,
}

/// Registry adapter for FFI providers that need to register with event registry
struct RegistryAdapter {
    // This will be filled with the actual event registry when needed
    registry: Option<Arc<EventRegistry>>,
}

impl RegistryAdapter {
    fn new() -> Self {
        Self { registry: None }
    }
    
    fn set_registry(&mut self, registry: Arc<EventRegistry>) {
        self.registry = Some(registry);
    }
}

// FFI-compatible registry interface for providers
#[repr(C)]
pub struct FFIRegistryInterface {
    pub register_fn: unsafe extern "C" fn(
        registry: *mut std::ffi::c_void,
        event_name: *const std::os::raw::c_char,
        handler: unsafe extern "C" fn(data: *const std::os::raw::c_char) -> *const std::os::raw::c_char,
    ),
}

/// Adapter for legacy CPI plugins
struct LegacyCpiAdapter {
    name: String,
    file_path: String,
}

impl LegacyCpiAdapter {
    fn new(file_path: String) -> Self {
        // Extract name from file path
        let name = Path::new(&file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
            
        Self { name, file_path }
    }
}

impl Provider for LegacyCpiAdapter {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn features(&self) -> Vec<String> {
        // Legacy CPIs don't expose features through the provider interface
        // Features are separate interface definitions loaded independently
        vec![]
    }
    
    fn feature_operations(&self, feature: &str) -> ProviderResult<Vec<String>> {
        // TODO: Extract from actual CPI
        match feature {
            "vm-management" => Ok(vec!["start".to_string(), "stop".to_string(), "create".to_string()]),
            "vm-control" => Ok(vec!["reset".to_string(), "pause".to_string()]),
            _ => Err(ProviderError::FeatureNotSupported {
                provider: self.name.clone(),
                feature: feature.to_string(),
            }),
        }
    }
    
    fn execute_operation(
        &self,
        feature: &str,
        operation: &str,
        args: HashMap<String, serde_json::Value>,
        context: &dyn ProviderContext,
    ) -> ProviderResult<serde_json::Value> {
        // TODO: Route to actual CPI execution
        Ok(serde_json::json!({
            "success": true,
            "provider": self.name,
            "feature": feature,
            "operation": operation,
            "message": "Legacy CPI operation executed"
        }))
    }
    
    fn initialize(&mut self, context: &dyn ProviderContext) -> ProviderResult<()> {
        Ok(())
    }
    
    fn shutdown(&mut self, context: &dyn ProviderContext) -> ProviderResult<()> {
        Ok(())
    }
}

/// Wrapper for modern providers loaded via FFI
struct ModernProviderWrapper {
    provider_ptr: *mut std::ffi::c_void,
    metadata: serde_json::Value,
    file_path: std::path::PathBuf,
}

impl ModernProviderWrapper {
    fn new(provider_ptr: *mut std::ffi::c_void, metadata: serde_json::Value, file_path: std::path::PathBuf) -> Self {
        Self {
            provider_ptr,
            metadata,
            file_path,
        }
    }
}

unsafe impl Send for ModernProviderWrapper {}
unsafe impl Sync for ModernProviderWrapper {}

impl Provider for ModernProviderWrapper {
    fn name(&self) -> &str {
        self.metadata.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    }
    
    fn version(&self) -> &str {
        self.metadata.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
    }
    
    fn features(&self) -> Vec<String> {
        // Modern providers don't expose features through the provider interface
        // Features are separate interface definitions loaded independently
        vec![]
    }
    
    fn feature_operations(&self, feature: &str) -> ProviderResult<Vec<String>> {
        // For modern providers, operations are registered dynamically with the event registry
        // We don't need to enumerate them here as they're handled by the registry
        Ok(vec![])
    }
    
    fn execute_operation(
        &self,
        feature: &str,
        operation: &str,
        args: HashMap<String, serde_json::Value>,
        context: &dyn ProviderContext,
    ) -> ProviderResult<serde_json::Value> {
        // Modern providers use event registry for execution, not direct calls
        Err(ProviderError::ExecutionFailed(
            "Modern providers should be executed through event registry".to_string()
        ))
    }
    
    fn initialize(&mut self, context: &dyn ProviderContext) -> ProviderResult<()> {
        // Modern providers are initialized when loaded and registered with event registry
        Ok(())
    }
    
    fn shutdown(&mut self, context: &dyn ProviderContext) -> ProviderResult<()> {
        // TODO: Call provider's shutdown function via FFI if available
        Ok(())
    }
}

/// Adapter for feature plugins
struct FeatureAdapter {
    name: String,
    operations: Vec<String>,
    file_path: String,
}

impl FeatureAdapter {
    fn new(name: String, operations: Vec<String>, file_path: String) -> Self {
        Self { name, operations, file_path }
    }
}

impl Provider for FeatureAdapter {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn features(&self) -> Vec<String> {
        vec![self.name.clone()]
    }
    
    fn feature_operations(&self, feature: &str) -> ProviderResult<Vec<String>> {
        if feature == self.name {
            Ok(self.operations.clone())
        } else {
            Err(ProviderError::FeatureNotSupported {
                provider: self.name.clone(),
                feature: feature.to_string(),
            })
        }
    }
    
    fn execute_operation(
        &self,
        feature: &str,
        operation: &str,
        args: HashMap<String, serde_json::Value>,
        context: &dyn ProviderContext,
    ) -> ProviderResult<serde_json::Value> {
        if feature != self.name {
            return Err(ProviderError::FeatureNotSupported {
                provider: self.name.clone(),
                feature: feature.to_string(),
            });
        }
        
        if !self.operations.contains(&operation.to_string()) {
            return Err(ProviderError::OperationNotSupported {
                provider: self.name.clone(),
                feature: feature.to_string(),
                operation: operation.to_string(),
            });
        }
        
        // TODO: Route to actual feature execution via FFI
        Ok(serde_json::json!({
            "success": true,
            "provider": self.name,
            "feature": feature,
            "operation": operation,
            "args": args,
            "message": "Feature operation executed"
        }))
    }
    
    fn initialize(&mut self, context: &dyn ProviderContext) -> ProviderResult<()> {
        Ok(())
    }
    
    fn shutdown(&mut self, context: &dyn ProviderContext) -> ProviderResult<()> {
        Ok(())
    }
}
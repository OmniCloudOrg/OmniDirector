//! # Provider Loader
//!
//! Handles dynamic loading of providers from shared libraries.

use super::{Provider, ProviderError, ProviderResult, ProviderMetadata, ProviderContext};
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
                match self.load_provider_from_file(&path, Arc::clone(&context)).await {
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
                self.load_legacy_cpi(&lib, library_path, context).await
            }
            ProviderType::ModernProvider => {
                self.load_modern_provider(&lib, library_path, context).await
            }
            ProviderType::Feature => {
                self.load_feature_as_provider(&lib, library_path, context).await
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
    
    /// Load a legacy CPI and wrap it as a provider
    async fn load_legacy_cpi(
        &self,
        lib: &Library,
        library_path: &Path,
        context: Arc<dyn ProviderContext>,
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
            features: Vec::new(), // TODO: Extract from CPI
            settings_schema: None,
            file_path: Some(library_path.to_string_lossy().to_string()),
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
    ) -> ProviderResult<(Arc<dyn Provider>, ProviderMetadata)> {
        println!("Detected modern provider");
        
        // TODO: Implement modern provider loading
        Err(ProviderError::LoadingFailed(
            "Modern provider loading not yet implemented".to_string()
        ))
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
        let feature_metadata = super::FeatureMetadata {
            name: feature_name.clone(),
            description: format!("{} feature", feature_name),
            version: "1.0.0".to_string(),
            operations: operations.into_iter().map(|op| super::OperationMetadata {
                name: op,
                description: "Feature operation".to_string(),
                arguments: Vec::new(), // TODO: Extract from feature
                return_type: "Value".to_string(),
                is_mutating: false,
                estimated_duration_ms: None,
            }).collect(),
        };
        
        let metadata = ProviderMetadata {
            name: format!("feature-{}", feature_name),
            version: "1.0.0".to_string(),
            description: format!("Feature provider for {}", feature_name),
            author: None,
            license: None,
            features: vec![feature_metadata],
            settings_schema: None,
            file_path: Some(library_path.to_string_lossy().to_string()),
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
        // TODO: Extract from actual CPI
        vec!["vm-management".to_string(), "vm-control".to_string()]
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
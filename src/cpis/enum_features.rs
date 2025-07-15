//! # Enum-Based Feature System
//!
//! Clean enum-based feature system that replaces the verbose trait-based approach.
//! Features are defined as enums with type-safe parameters pulled from value pools.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use serde_json::Value;
use super::{PluginError, FeatureContext};
use super::context::LogLevel;

/// Arguments for feature operations - tuple of name and type
pub type Args = Vec<(String, String)>;

/// Dynamic library loader for feature crates
#[derive(Debug)]
pub struct FeatureLoader {
    libraries: HashMap<String, libloading::Library>,
}

impl FeatureLoader {
    pub fn new() -> Self {
        Self {
            libraries: HashMap::new(),
        }
    }

    /// Load a feature from a dynamic library
    pub fn load_feature(&mut self, feature_name: &str, lib_path: &str) -> Result<(), PluginError> {
        unsafe {
            let lib = libloading::Library::new(lib_path)
                .map_err(|e| PluginError::InitializationFailed(format!("Failed to load library {}: {}", lib_path, e)))?;
            
            // Get the feature name function
            let get_name: libloading::Symbol<extern "C" fn() -> *const std::os::raw::c_char> = 
                lib.get(b"get_feature_name")
                    .map_err(|e| PluginError::InitializationFailed(format!("Missing get_feature_name function: {}", e)))?;
            
            let name_ptr = get_name();
            let name = std::ffi::CStr::from_ptr(name_ptr).to_str()
                .map_err(|e| PluginError::InitializationFailed(format!("Invalid feature name: {}", e)))?;
            
            if name != feature_name {
                return Err(PluginError::InitializationFailed(format!("Feature name mismatch: expected {}, got {}", feature_name, name)));
            }
            
            self.libraries.insert(feature_name.to_string(), lib);
            
            println!("✅ Loaded feature: {}", feature_name);
            Ok(())
        }
    }

    /// Execute an operation on a loaded feature
    pub fn execute_operation(
        &self,
        feature_name: &str,
        operation: &str,
        args: &HashMap<String, Value>,
    ) -> Result<Value, PluginError> {
        let lib = self.libraries.get(feature_name)
            .ok_or_else(|| PluginError::UnsupportedFeature(feature_name.to_string()))?;

        unsafe {
            let execute: libloading::Symbol<extern "C" fn(*const std::os::raw::c_char, *const std::os::raw::c_char) -> *const std::os::raw::c_char> = 
                lib.get(b"execute_operation")
                    .map_err(|e| PluginError::ExecutionFailed(format!("Missing execute_operation function: {}", e)))?;
            
            let operation_cstr = std::ffi::CString::new(operation)
                .map_err(|e| PluginError::InvalidArgument(format!("Invalid operation name: {}", e)))?;
            
            let args_json = serde_json::to_string(args)
                .map_err(|e| PluginError::InvalidArgument(format!("Failed to serialize args: {}", e)))?;
            
            let args_cstr = std::ffi::CString::new(args_json)
                .map_err(|e| PluginError::InvalidArgument(format!("Invalid args JSON: {}", e)))?;
            
            let result_ptr = execute(operation_cstr.as_ptr(), args_cstr.as_ptr());
            let result_str = std::ffi::CStr::from_ptr(result_ptr).to_str()
                .map_err(|e| PluginError::ExecutionFailed(format!("Invalid result: {}", e)))?;
            
            serde_json::from_str(result_str)
                .map_err(|e| PluginError::ExecutionFailed(format!("Failed to parse result: {}", e)))
        }
    }

    /// Get available operations for a feature
    pub fn get_operations(&self, feature_name: &str) -> Result<Vec<String>, PluginError> {
        let lib = self.libraries.get(feature_name)
            .ok_or_else(|| PluginError::UnsupportedFeature(feature_name.to_string()))?;

        unsafe {
            let get_ops: libloading::Symbol<extern "C" fn() -> *const std::os::raw::c_char> = 
                lib.get(b"register_operations")
                    .map_err(|e| PluginError::ExecutionFailed(format!("Missing register_operations function: {}", e)))?;
            
            let ops_ptr = get_ops();
            let ops_str = std::ffi::CStr::from_ptr(ops_ptr).to_str()
                .map_err(|e| PluginError::ExecutionFailed(format!("Invalid operations JSON: {}", e)))?;
            
            serde_json::from_str(ops_str)
                .map_err(|e| PluginError::ExecutionFailed(format!("Failed to parse operations: {}", e)))
        }
    }
}

/// Enhanced feature manager that handles both enum-based and legacy features
#[derive(Debug)]
pub struct EnhancedFeatureManager {
    pub feature_loader: FeatureLoader,
    pub loaded_features: HashMap<String, Vec<String>>, // feature_name -> operations
}

impl EnhancedFeatureManager {
    pub fn new() -> Self {
        Self {
            feature_loader: FeatureLoader::new(),
            loaded_features: HashMap::new(),
        }
    }

    /// Initialize with default features
    pub async fn initialize(&mut self) -> Result<(), PluginError> {
        // Load features from the features directory
        self.load_default_features().await?;
        
        println!("✅ Enhanced feature manager initialized with {} features", self.loaded_features.len());
        Ok(())
    }

    /// Load default features from the features directory
    async fn load_default_features(&mut self) -> Result<(), PluginError> {
        let features_dir = std::path::Path::new("./features");
        
        if !features_dir.exists() {
            tokio::fs::create_dir_all(features_dir).await?;
            return Ok(());
        }

        // For development, we'll simulate loading features
        // In production, this would scan for .dll/.so files
        self.register_builtin_features().await?;
        
        Ok(())
    }

    /// Register built-in features for development (disabled - using plugins only)
    async fn register_builtin_features(&mut self) -> Result<(), PluginError> {
        // No built-in features - all functionality comes from plugins
        println!("📦 No built-in features registered - using plugins only");
        Ok(())
    }

    /// Execute a feature operation
    pub async fn execute_operation(
        &self,
        feature_name: &str,
        operation: &str,
        args: HashMap<String, Value>,
        context: &dyn FeatureContext,
    ) -> Result<Value, PluginError> {
        let operations = self.loaded_features.get(feature_name)
            .ok_or_else(|| PluginError::UnsupportedFeature(feature_name.to_string()))?;

        if !operations.contains(&operation.to_string()) {
            return Err(PluginError::InvalidArgument(format!("Operation {} not found in feature {}", operation, feature_name)));
        }

        context.log(LogLevel::Info, &format!("Executing {} operation on {} feature", operation, feature_name)).await;

        // For now, simulate execution with type-safe responses
        let result = self.simulate_operation_execution(feature_name, operation, args, context).await?;
        
        context.log(LogLevel::Info, &format!("Operation {} completed successfully", operation)).await;
        Ok(result)
    }

    /// Simulate operation execution (replace with actual execution in production)
    async fn simulate_operation_execution(
        &self,
        feature_name: &str,
        operation: &str,
        args: HashMap<String, Value>,
        context: &dyn FeatureContext,
    ) -> Result<Value, PluginError> {
        match (feature_name, operation) {
            ("worker_management", "StartWorker") => {
                let worker_name = args.get("worker_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| PluginError::InvalidArgument("worker_name is required".to_string()))?;

                // Resolve values from context
                let instance_type = self.resolve_value_from_context("instance_type", &args, context).await?;
                let availability_zone = self.resolve_value_from_context("availability_zone", &args, context).await.unwrap_or_else(|_| "us-east-1a".to_string());

                Ok(serde_json::json!({
                    "worker_id": Uuid::new_v4().to_string(),
                    "name": worker_name,
                    "state": "Creating",
                    "instance_type": instance_type,
                    "availability_zone": availability_zone,
                    "created_at": chrono::Utc::now().to_rfc3339(),
                    "message": "Worker creation initiated"
                }))
            },
            ("worker_management", "StopWorker") => {
                let worker_id = args.get("worker_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| PluginError::InvalidArgument("worker_id is required".to_string()))?;

                Ok(serde_json::json!({
                    "worker_id": worker_id,
                    "state": "Stopping",
                    "message": "Worker stop initiated"
                }))
            },
            ("worker_management", "ListWorkers") => {
                Ok(serde_json::json!({
                    "workers": [],
                    "count": 0,
                    "message": "No workers found"
                }))
            },
            ("vm_management", "CreateVM") => {
                let vm_name = args.get("vm_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| PluginError::InvalidArgument("vm_name is required".to_string()))?;

                Ok(serde_json::json!({
                    "vm_id": Uuid::new_v4().to_string(),
                    "name": vm_name,
                    "state": "Creating",
                    "created_at": chrono::Utc::now().to_rfc3339(),
                    "message": "VM creation initiated"
                }))
            },
            _ => Err(PluginError::InvalidArgument(format!("Operation {} not implemented for feature {}", operation, feature_name)))
        }
    }

    /// Resolve a value from the context using the value pool system
    async fn resolve_value_from_context(
        &self,
        key: &str,
        args: &HashMap<String, Value>,
        context: &dyn FeatureContext,
    ) -> Result<String, PluginError> {
        // 1. Check direct arguments first
        if let Some(value) = args.get(key) {
            if let Some(str_value) = value.as_str() {
                return Ok(str_value.to_string());
            }
        }

        // 2. Check execution context
        if let Some(context_value) = context.get_value(key).await {
            if let Some(str_value) = context_value.as_str() {
                return Ok(str_value.to_string());
            }
        }

        // 3. Check environment variables
        if let Some(env_value) = context.get_env(key).await {
            return Ok(env_value);
        }

        // 4. Use defaults based on key
        match key {
            "instance_type" => Ok("t3.medium".to_string()),
            "availability_zone" => Ok("us-east-1a".to_string()),
            "image" => Ok("ami-0c02fb55956c7d316".to_string()),
            _ => Err(PluginError::InvalidArgument(format!("Required parameter '{}' not found in any value pool", key)))
        }
    }

    /// Get available features
    pub fn get_available_features(&self) -> Vec<String> {
        self.loaded_features.keys().cloned().collect()
    }

    /// Get available operations for a feature
    pub fn get_feature_operations(&self, feature_name: &str) -> Result<Vec<String>, PluginError> {
        self.loaded_features.get(feature_name)
            .cloned()
            .ok_or_else(|| PluginError::UnsupportedFeature(feature_name.to_string()))
    }

    /// Get argument definitions for an operation
    pub fn get_operation_arguments(&self, feature_name: &str, operation: &str) -> Result<Args, PluginError> {
        match (feature_name, operation) {
            ("worker_management", "StartWorker") => Ok(vec![
                ("worker_name".to_string(), "String".to_string()),
                ("instance_type".to_string(), "String".to_string()),
                ("availability_zone".to_string(), "String".to_string()),
                ("image".to_string(), "String".to_string()),
                ("security_group".to_string(), "String".to_string()),
                ("network".to_string(), "String".to_string()),
            ]),
            ("worker_management", "StopWorker") => Ok(vec![
                ("worker_id".to_string(), "String".to_string()),
            ]),
            ("worker_management", "DeleteWorker") => Ok(vec![
                ("worker_id".to_string(), "String".to_string()),
            ]),
            ("worker_management", "ListWorkers") => Ok(vec![]),
            ("worker_management", "GetWorkerStatus") => Ok(vec![
                ("worker_id".to_string(), "String".to_string()),
            ]),
            ("worker_management", "ScaleWorkers") => Ok(vec![
                ("target_count".to_string(), "i32".to_string()),
                ("instance_type".to_string(), "String".to_string()),
            ]),
            ("vm_management", "CreateVM") => Ok(vec![
                ("vm_name".to_string(), "String".to_string()),
                ("instance_type".to_string(), "String".to_string()),
                ("image".to_string(), "String".to_string()),
            ]),
            _ => Err(PluginError::InvalidArgument(format!("Operation {} not found in feature {}", operation, feature_name)))
        }
    }
}

/// Statistics for the enhanced feature system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedFeatureStats {
    pub total_features: usize,
    pub total_operations: usize,
    pub features: HashMap<String, usize>, // feature_name -> operation_count
}

impl EnhancedFeatureManager {
    pub fn get_stats(&self) -> EnhancedFeatureStats {
        let mut features = HashMap::new();
        let mut total_operations = 0;

        for (feature_name, operations) in &self.loaded_features {
            let op_count = operations.len();
            features.insert(feature_name.clone(), op_count);
            total_operations += op_count;
        }

        EnhancedFeatureStats {
            total_features: self.loaded_features.len(),
            total_operations,
            features,
        }
    }
}
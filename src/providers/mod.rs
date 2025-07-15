//! # Provider System
//!
//! Manages external service providers (VirtualBox, AWS, Docker, etc.)
//! Providers expose features that can be used by the system.

pub mod registry;
pub mod loader;
pub mod metadata;
pub mod context;

pub use registry::*;
pub use loader::*;
pub use metadata::*;
pub use context::*;

use std::collections::HashMap;
use serde_json::Value;
use thiserror::Error;

/// Provider system errors
#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Provider not found: {0}")]
    NotFound(String),
    
    #[error("Provider loading failed: {0}")]
    LoadingFailed(String),
    
    #[error("Provider initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Provider execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Feature not supported: {feature} in provider {provider}")]
    FeatureNotSupported { provider: String, feature: String },
    
    #[error("Operation not supported: {operation} in {provider}/{feature}")]
    OperationNotSupported { provider: String, feature: String, operation: String },
    
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    
    #[error("Invalid route: {0}")]
    InvalidRoute(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Library loading error: {0}")]
    LibraryError(#[from] libloading::Error),
}

/// Result type for provider operations
pub type ProviderResult<T> = Result<T, ProviderError>;

/// Provider capability trait
pub trait Provider: Send + Sync {
    /// Get provider name
    fn name(&self) -> &str;
    
    /// Get provider version
    fn version(&self) -> &str;
    
    /// Get supported features
    fn features(&self) -> Vec<String>;
    
    /// Check if provider supports a feature
    fn supports_feature(&self, feature: &str) -> bool {
        self.features().contains(&feature.to_string())
    }
    
    /// Get operations for a feature
    fn feature_operations(&self, feature: &str) -> ProviderResult<Vec<String>>;
    
    /// Execute an operation
    fn execute_operation(
        &self,
        feature: &str,
        operation: &str,
        args: HashMap<String, Value>,
        context: &dyn ProviderContext,
    ) -> ProviderResult<Value>;
    
    /// Initialize provider
    fn initialize(&mut self, context: &dyn ProviderContext) -> ProviderResult<()>;
    
    /// Shutdown provider
    fn shutdown(&mut self, context: &dyn ProviderContext) -> ProviderResult<()>;
}
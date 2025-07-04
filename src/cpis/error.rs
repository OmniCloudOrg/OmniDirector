use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Plugin runtime error: {0}")]
    Runtime(String),
    
    #[error("Event system error: {0}")]
    EventSystem(String),
    
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
    
    #[error("Feature not supported: {0}")]
    FeatureNotSupported(String),
    
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),
    
    #[error("Invalid parameter type for {param}: expected {expected}, got {actual}")]
    InvalidParameterType {
        param: String,
        expected: String,
        actual: String,
    },
    
    #[error("Event handler execution failed: {0}")]
    HandlerExecution(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Dynamic library error: {0}")]
    LibLoading(String),
    
    #[error("Plugin validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Timeout waiting for event response: {0}")]
    Timeout(String),
    
    #[error("Plugin dependency not satisfied: {0}")]
    DependencyNotSatisfied(String),
}

impl From<libloading::Error> for PluginError {
    fn from(err: libloading::Error) -> Self {
        PluginError::LibLoading(err.to_string())
    }
}

pub type PluginResult<T> = Result<T, PluginError>;

/// Plugin-specific error with context
#[derive(Debug)]
pub struct PluginErrorContext {
    pub plugin_name: String,
    pub feature: Option<String>,
    pub event_key: Option<String>,
    pub error: PluginError,
}

impl fmt::Display for PluginErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Plugin '{}': ", self.plugin_name)?;
        
        if let Some(feature) = &self.feature {
            write!(f, "Feature '{}': ", feature)?;
        }
        
        if let Some(event_key) = &self.event_key {
            write!(f, "Event '{}': ", event_key)?;
        }
        
        write!(f, "{}", self.error)
    }
}

impl std::error::Error for PluginErrorContext {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Helper trait for adding context to plugin errors
pub trait PluginErrorExt<T> {
    fn with_plugin_context(
        self,
        plugin_name: impl Into<String>,
        feature: Option<impl Into<String>>,
        event_key: Option<impl Into<String>>,
    ) -> Result<T, PluginErrorContext>;
}

impl<T> PluginErrorExt<T> for PluginResult<T> {
    fn with_plugin_context(
        self,
        plugin_name: impl Into<String>,
        feature: Option<impl Into<String>>,
        event_key: Option<impl Into<String>>,
    ) -> Result<T, PluginErrorContext> {
        self.map_err(|error| PluginErrorContext {
            plugin_name: plugin_name.into(),
            feature: feature.map(|f| f.into()),
            event_key: event_key.map(|e| e.into()),
            error,
        })
    }
}
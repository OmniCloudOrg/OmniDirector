//! # Provider Context
//!
//! Execution context for providers, providing access to system services.

use std::collections::HashMap;
use serde_json::Value;
use async_trait::async_trait;

/// Log levels for provider logging
#[derive(Debug, Clone)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Provider execution context
#[async_trait]
pub trait ProviderContext: Send + Sync {
    /// Log a message
    async fn log(&self, level: LogLevel, message: &str);
    
    /// Get a configuration value
    async fn get_config(&self, key: &str) -> Option<Value>;
    
    /// Set a configuration value
    async fn set_config(&self, key: &str, value: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Get system information
    async fn get_system_info(&self) -> HashMap<String, Value>;
    
    /// Get provider-specific settings
    async fn get_provider_settings(&self, provider: &str) -> HashMap<String, Value>;
    
    /// Store data in the context
    async fn store_data(&self, key: &str, value: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Retrieve data from the context
    async fn get_data(&self, key: &str) -> Option<Value>;
    
    /// Call another provider/feature/operation
    async fn call_operation(
        &self,
        provider: &str,
        feature: &str,
        operation: &str,
        args: HashMap<String, Value>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
}

/// Default implementation of provider context
#[derive(Debug)]
pub struct DefaultProviderContext {
    config: tokio::sync::RwLock<HashMap<String, Value>>,
    data: tokio::sync::RwLock<HashMap<String, Value>>,
    provider_settings: HashMap<String, HashMap<String, Value>>,
}

impl DefaultProviderContext {
    /// Create a new default provider context
    pub fn new() -> Self {
        Self {
            config: tokio::sync::RwLock::new(HashMap::new()),
            data: tokio::sync::RwLock::new(HashMap::new()),
            provider_settings: HashMap::new(),
        }
    }
    
    /// Create with provider settings
    pub fn with_provider_settings(provider_settings: HashMap<String, HashMap<String, Value>>) -> Self {
        Self {
            config: tokio::sync::RwLock::new(HashMap::new()),
            data: tokio::sync::RwLock::new(HashMap::new()),
            provider_settings,
        }
    }
}

#[async_trait]
impl ProviderContext for DefaultProviderContext {
    async fn log(&self, level: LogLevel, message: &str) {
        let level_str = match level {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        };
        
        println!("[{}] {}", level_str, message);
    }
    
    async fn get_config(&self, key: &str) -> Option<Value> {
        let config = self.config.read().await;
        config.get(key).cloned()
    }
    
    async fn set_config(&self, key: &str, value: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut config = self.config.write().await;
        config.insert(key.to_string(), value);
        Ok(())
    }
    
    async fn get_system_info(&self) -> HashMap<String, Value> {
        let mut info = HashMap::new();
        info.insert("hostname".to_string(), Value::String(whoami::hostname()));
        info.insert("username".to_string(), Value::String(whoami::username()));
        info.insert("platform".to_string(), Value::String(std::env::consts::OS.to_string()));
        info.insert("arch".to_string(), Value::String(std::env::consts::ARCH.to_string()));
        info
    }
    
    async fn get_provider_settings(&self, provider: &str) -> HashMap<String, Value> {
        self.provider_settings.get(provider).cloned().unwrap_or_default()
    }
    
    async fn store_data(&self, key: &str, value: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value);
        Ok(())
    }
    
    async fn get_data(&self, key: &str) -> Option<Value> {
        let data = self.data.read().await;
        data.get(key).cloned()
    }
    
    async fn call_operation(
        &self,
        provider: &str,
        feature: &str,
        operation: &str,
        args: HashMap<String, Value>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        // This would be implemented to route to the actual provider system
        // For now, return a placeholder
        Err(format!(
            "Cross-provider calls not yet implemented: {}/{}/{}",
            provider, feature, operation
        ).into())
    }
}
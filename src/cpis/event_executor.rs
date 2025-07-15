use omni_event_registry::*;
use serde_json::Value;
use std::sync::Arc;
use super::PluginError;
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Enhanced executor that uses the global event registry
#[derive(Debug)]
pub struct EventDrivenExecutor {
    /// Track loaded providers for validation
    loaded_providers: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl EventDrivenExecutor {
    pub fn new() -> Self {
        Self {
            loaded_providers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a provider with its features
    pub async fn register_provider(&self, provider_name: String, features: Vec<String>) {
        let mut providers = self.loaded_providers.write().await;
        providers.insert(provider_name, features);
    }
    
    /// Execute a command using the event-driven system
    pub async fn execute_command(
        &self,
        provider: &str,
        feature: &str,
        method: &str,
        payload: Value,
    ) -> Result<Value, PluginError> {
        println!("🎯 Executing via event system: {}::{}::{}", provider, feature, method);
        
        // Dispatch to the global event registry (NO MATCH STATEMENTS!)
        match dispatch_event(provider, feature, method, payload).await {
            Ok(result) => {
                println!("✅ Event executed successfully");
                Ok(result)
            },
            Err(EventError::HandlerNotFound(handler)) => {
                println!("❌ Handler not found: {}", handler);
                Err(PluginError::ExecutionFailed(format!("Handler not found: {}", handler)))
            },
            Err(EventError::ExecutionFailed(msg)) => {
                println!("❌ Execution failed: {}", msg);
                Err(PluginError::ExecutionFailed(msg))
            },
            Err(EventError::InvalidPayload(msg)) => {
                println!("❌ Invalid payload: {}", msg);
                Err(PluginError::ExecutionFailed(format!("Invalid payload: {}", msg)))
            },
        }
    }
    
    /// List all available handlers
    pub fn list_available_handlers(&self) -> Vec<String> {
        get_global_registry().list_handlers()
    }
    
    /// Get loaded providers
    pub async fn get_loaded_providers(&self) -> HashMap<String, Vec<String>> {
        self.loaded_providers.read().await.clone()
    }
    
    /// Validate that a provider supports a feature
    pub async fn validate_provider_feature(&self, provider: &str, feature: &str) -> bool {
        let providers = self.loaded_providers.read().await;
        if let Some(features) = providers.get(provider) {
            features.contains(&feature.to_string())
        } else {
            false
        }
    }
}
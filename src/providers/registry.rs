//! # Provider Registry
//!
//! Centralized registry for all providers, handling discovery, loading, and lifecycle management.

use super::{Provider, ProviderError, ProviderResult, ProviderContext, ProviderMetadata};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Provider registry manages all loaded providers
pub struct ProviderRegistry {
    /// Map of provider name to provider instance
    providers: RwLock<HashMap<String, Arc<dyn Provider>>>,
    /// Map of provider name to metadata
    metadata: RwLock<HashMap<String, ProviderMetadata>>,
    /// Provider context for execution
    context: Arc<dyn ProviderContext>,
}

impl ProviderRegistry {
    /// Create a new provider registry
    pub fn new(context: Arc<dyn ProviderContext>) -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            metadata: RwLock::new(HashMap::new()),
            context,
        }
    }
    
    /// Register a provider
    pub async fn register_provider(
        &self,
        provider: Arc<dyn Provider>,
        metadata: ProviderMetadata,
    ) -> ProviderResult<()> {
        let name = provider.name().to_string();
        
        // Check if provider already exists
        {
            let providers = self.providers.read().await;
            if providers.contains_key(&name) {
                return Err(ProviderError::InitializationFailed(
                    format!("Provider '{}' already registered", name)
                ));
            }
        }
        
        // Register provider and metadata
        {
            let mut providers = self.providers.write().await;
            let mut metadata_store = self.metadata.write().await;
            
            providers.insert(name.clone(), provider);
            metadata_store.insert(name.clone(), metadata);
        }
        
        println!("✅ Registered provider: {}", name);
        Ok(())
    }
    
    /// Get a provider by name
    pub async fn get_provider(&self, name: &str) -> Option<Arc<dyn Provider>> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }
    
    /// Get provider metadata
    pub async fn get_metadata(&self, name: &str) -> Option<ProviderMetadata> {
        let metadata = self.metadata.read().await;
        metadata.get(name).cloned()
    }
    
    /// List all provider names
    pub async fn list_providers(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
    
    /// Check if provider exists
    pub async fn has_provider(&self, name: &str) -> bool {
        let providers = self.providers.read().await;
        providers.contains_key(name)
    }
    
    /// Get all provider metadata
    pub async fn list_metadata(&self) -> Vec<ProviderMetadata> {
        let metadata = self.metadata.read().await;
        metadata.values().cloned().collect()
    }
    
    /// Execute an operation on a provider
    pub async fn execute_operation(
        &self,
        provider_name: &str,
        feature: &str,
        operation: &str,
        args: HashMap<String, serde_json::Value>,
    ) -> ProviderResult<serde_json::Value> {
        let provider = self.get_provider(provider_name).await
            .ok_or_else(|| ProviderError::NotFound(provider_name.to_string()))?;
        
        // Check if provider supports the feature
        if !provider.supports_feature(feature) {
            return Err(ProviderError::FeatureNotSupported {
                provider: provider_name.to_string(),
                feature: feature.to_string(),
            });
        }
        
        // Check if feature supports the operation
        let operations = provider.feature_operations(feature)?;
        if !operations.contains(&operation.to_string()) {
            return Err(ProviderError::OperationNotSupported {
                provider: provider_name.to_string(),
                feature: feature.to_string(),
                operation: operation.to_string(),
            });
        }
        
        // Execute the operation
        provider.execute_operation(feature, operation, args, self.context.as_ref())
    }
    
    /// Get operations for a feature
    pub async fn get_feature_operations(
        &self,
        provider_name: &str,
        feature: &str,
    ) -> ProviderResult<Vec<String>> {
        let provider = self.get_provider(provider_name).await
            .ok_or_else(|| ProviderError::NotFound(provider_name.to_string()))?;
        
        if !provider.supports_feature(feature) {
            return Err(ProviderError::FeatureNotSupported {
                provider: provider_name.to_string(),
                feature: feature.to_string(),
            });
        }
        
        provider.feature_operations(feature)
    }
    
    /// Get registry statistics
    pub async fn get_statistics(&self) -> ProviderRegistryStats {
        let providers = self.providers.read().await;
        let metadata = self.metadata.read().await;
        
        let mut total_features = 0;
        let mut total_operations = 0;
        let mut provider_stats = HashMap::new();
        
        for (name, meta) in metadata.iter() {
            let feature_count = meta.features.len();
            let operation_count: usize = meta.features.iter()
                .map(|f| f.operations.len())
                .sum();
            
            total_features += feature_count;
            total_operations += operation_count;
            
            provider_stats.insert(name.clone(), ProviderStats {
                feature_count,
                operation_count,
                supported_features: meta.feature_names(),
            });
        }
        
        ProviderRegistryStats {
            total_providers: providers.len(),
            total_features,
            total_operations,
            provider_stats,
        }
    }
    
    /// Initialize all providers
    pub async fn initialize_all(&self) -> ProviderResult<()> {
        let providers = self.providers.read().await;
        let errors: Vec<String> = Vec::new();
        
        for (name, _provider) in providers.iter() {
            // We need to get a mutable reference, but we can't do that with Arc<dyn Provider>
            // This is a limitation of the current design - we'll need to handle initialization differently
            println!("🔧 Initializing provider: {}", name);
            // TODO: Implement proper initialization pattern
        }
        
        if !errors.is_empty() {
            return Err(ProviderError::InitializationFailed(
                format!("Failed to initialize {} providers", errors.len())
            ));
        }
        
        Ok(())
    }
    
    /// Shutdown all providers
    pub async fn shutdown_all(&self) -> ProviderResult<()> {
        let providers = self.providers.read().await;
        let errors: Vec<String> = Vec::new();
        
        for (name, _provider) in providers.iter() {
            println!("🛑 Shutting down provider: {}", name);
            // TODO: Implement proper shutdown pattern
        }
        
        if !errors.is_empty() {
            return Err(ProviderError::ExecutionFailed(
                format!("Failed to shutdown {} providers", errors.len())
            ));
        }
        
        Ok(())
    }
}

/// Provider registry statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderRegistryStats {
    pub total_providers: usize,
    pub total_features: usize,
    pub total_operations: usize,
    pub provider_stats: HashMap<String, ProviderStats>,
}

/// Individual provider statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderStats {
    pub feature_count: usize,
    pub operation_count: usize,
    pub supported_features: Vec<String>,
}
//! # Request Router
//!
//! Central router that handles all incoming requests and routes them to the appropriate providers.

use super::{Route, resolver::RouteResolver};
use crate::providers::{ProviderRegistry, ProviderError, ProviderResult};
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::Value;

/// Router handles all incoming requests and routes them to providers
pub struct Router {
    /// Provider registry
    registry: Arc<ProviderRegistry>,
    /// Route resolver for finding providers
    resolver: RouteResolver,
}

impl Router {
    /// Create a new router
    pub fn new(registry: Arc<ProviderRegistry>) -> Self {
        Self {
            resolver: RouteResolver::new(Arc::clone(&registry)),
            registry,
        }
    }

    /// Route a request to the appropriate provider
    pub async fn route_request(
        &self,
        route: Route,
        args: HashMap<String, Value>,
    ) -> ProviderResult<Value> {
        // Resolve the route to ensure provider/feature/operation exists
        self.resolver.resolve_route(&route).await?;

        // Execute the operation through the registry
        self.registry
            .execute_operation(&route.provider, &route.feature, &route.operation, args)
            .await
    }

    /// Route a request from a path string
    pub async fn route_path(
        &self,
        path: &str,
        args: HashMap<String, Value>,
    ) -> ProviderResult<Value> {
        let route = Route::from_path(path)
            .map_err(|e| ProviderError::InvalidRoute(e))?;
        
        self.route_request(route, args).await
    }

    /// Route a request from URL path (removes /providers/ prefix)
    pub async fn route_url_path(
        &self,
        url_path: &str,
        args: HashMap<String, Value>,
    ) -> ProviderResult<Value> {
        // Remove /providers/ prefix if present
        let path = if url_path.starts_with("/providers/") {
            &url_path[11..] // Remove "/providers/" (11 chars)
        } else {
            url_path
        };

        // Remove /features/ and /operations/ segments to get provider/feature/operation
        let normalized_path = path
            .replace("/features/", "/")
            .replace("/operations/", "/");

        self.route_path(&normalized_path, args).await
    }

    /// Get all available routes
    pub async fn get_available_routes(&self) -> ProviderResult<Vec<Route>> {
        self.resolver.list_all_routes().await
    }

    /// Get routes for a specific provider
    pub async fn get_provider_routes(&self, provider: &str) -> ProviderResult<Vec<Route>> {
        self.resolver.list_provider_routes(provider).await
    }

    /// Get routes for a specific feature
    pub async fn get_feature_routes(&self, provider: &str, feature: &str) -> ProviderResult<Vec<Route>> {
        self.resolver.list_feature_routes(provider, feature).await
    }

    /// Check if a route is valid
    pub async fn is_route_valid(&self, route: &Route) -> bool {
        self.resolver.resolve_route(route).await.is_ok()
    }

    /// Get route metadata
    pub async fn get_route_metadata(&self, route: &Route) -> ProviderResult<RouteMetadata> {
        // Validate route first
        self.resolver.resolve_route(route).await?;

        // Get provider metadata
        let provider_metadata = self.registry
            .get_metadata(&route.provider)
            .await
            .ok_or_else(|| ProviderError::NotFound(route.provider.clone()))?;

        // Find the feature
        let feature_metadata = provider_metadata
            .features
            .iter()
            .find(|f| f.name == route.feature)
            .ok_or_else(|| ProviderError::FeatureNotSupported {
                provider: route.provider.clone(),
                feature: route.feature.clone(),
            })?;

        // Find the operation
        let operation_metadata = feature_metadata
            .operations
            .iter()
            .find(|op| op.name == route.operation)
            .ok_or_else(|| ProviderError::OperationNotSupported {
                provider: route.provider.clone(),
                feature: route.feature.clone(),
                operation: route.operation.clone(),
            })?;

        Ok(RouteMetadata {
            route: route.clone(),
            provider_name: provider_metadata.name.clone(),
            provider_version: provider_metadata.version.clone(),
            feature_name: feature_metadata.name.clone(),
            feature_description: feature_metadata.description.clone(),
            operation_name: operation_metadata.name.clone(),
            operation_description: operation_metadata.description.clone(),
            operation_arguments: operation_metadata.arguments.clone(),
            operation_return_type: operation_metadata.return_type.clone(),
            is_mutating: operation_metadata.is_mutating,
            estimated_duration_ms: operation_metadata.estimated_duration_ms,
        })
    }
}

/// Metadata for a specific route
#[derive(Debug, Clone)]
pub struct RouteMetadata {
    pub route: Route,
    pub provider_name: String,
    pub provider_version: String,
    pub feature_name: String,
    pub feature_description: String,
    pub operation_name: String,
    pub operation_description: String,
    pub operation_arguments: Vec<crate::providers::ArgumentMetadata>,
    pub operation_return_type: String,
    pub is_mutating: bool,
    pub estimated_duration_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ProviderContext, DefaultProviderContext};

    #[tokio::test]
    async fn test_route_parsing() {
        let context = Arc::new(DefaultProviderContext::new());
        let registry = Arc::new(ProviderRegistry::new(context));
        let router = Router::new(registry);

        // Test URL path routing
        let result = router.route_url_path(
            "/providers/virtualbox/features/vm-management/operations/start-vm",
            HashMap::new()
        ).await;

        // Should fail because no providers are registered, but parsing should work
        assert!(matches!(result, Err(ProviderError::NotFound(_))));
    }

    #[test]
    fn test_url_path_normalization() {
        let path = "/providers/virtualbox/features/vm-management/operations/start-vm";
        let normalized = path
            .strip_prefix("/providers/").unwrap_or(path)
            .replace("/features/", "/")
            .replace("/operations/", "/");
        
        assert_eq!(normalized, "virtualbox/vm-management/start-vm");
    }
}
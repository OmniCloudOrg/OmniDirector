//! # Route Resolver
//!
//! Validates and resolves routes to ensure providers, features, and operations exist.

use super::Route;
use crate::providers::{ProviderRegistry, ProviderError, ProviderResult};
use std::sync::Arc;

/// Route resolver validates and resolves routes
pub struct RouteResolver {
    /// Provider registry for validation
    registry: Arc<ProviderRegistry>,
}

impl RouteResolver {
    /// Create a new route resolver
    pub fn new(registry: Arc<ProviderRegistry>) -> Self {
        Self { registry }
    }

    /// Resolve and validate a route
    pub async fn resolve_route(&self, route: &Route) -> ProviderResult<ResolvedRoute> {
        // Check if provider exists
        let provider = self.registry
            .get_provider(&route.provider)
            .await
            .ok_or_else(|| ProviderError::NotFound(route.provider.clone()))?;

        // Check if provider supports the feature
        if !provider.supports_feature(&route.feature) {
            return Err(ProviderError::FeatureNotSupported {
                provider: route.provider.clone(),
                feature: route.feature.clone(),
            });
        }

        // Check if feature supports the operation
        let operations = provider.feature_operations(&route.feature)?;
        if !operations.contains(&route.operation) {
            return Err(ProviderError::OperationNotSupported {
                provider: route.provider.clone(),
                feature: route.feature.clone(),
                operation: route.operation.clone(),
            });
        }

        Ok(ResolvedRoute {
            route: route.clone(),
            provider: provider.name().to_string(),
            feature: route.feature.clone(),
            operation: route.operation.clone(),
            available_operations: operations,
        })
    }

    /// List all available routes in the system
    pub async fn list_all_routes(&self) -> ProviderResult<Vec<Route>> {
        let mut routes = Vec::new();

        // Get all providers
        let provider_names = self.registry.list_providers().await;

        for provider_name in provider_names {
            if let Some(provider) = self.registry.get_provider(&provider_name).await {
                // Get all features for this provider
                let features = provider.features();

                for feature in features {
                    // Get all operations for this feature
                    if let Ok(operations) = provider.feature_operations(&feature) {
                        for operation in operations {
                            routes.push(Route::new(
                                provider_name.clone(),
                                feature.clone(),
                                operation,
                            ));
                        }
                    }
                }
            }
        }

        Ok(routes)
    }

    /// List routes for a specific provider
    pub async fn list_provider_routes(&self, provider_name: &str) -> ProviderResult<Vec<Route>> {
        let provider = self.registry
            .get_provider(provider_name)
            .await
            .ok_or_else(|| ProviderError::NotFound(provider_name.to_string()))?;

        let mut routes = Vec::new();
        let features = provider.features();

        for feature in features {
            if let Ok(operations) = provider.feature_operations(&feature) {
                for operation in operations {
                    routes.push(Route::new(
                        provider_name.to_string(),
                        feature.clone(),
                        operation,
                    ));
                }
            }
        }

        Ok(routes)
    }

    /// List routes for a specific feature
    pub async fn list_feature_routes(
        &self,
        provider_name: &str,
        feature_name: &str,
    ) -> ProviderResult<Vec<Route>> {
        let provider = self.registry
            .get_provider(provider_name)
            .await
            .ok_or_else(|| ProviderError::NotFound(provider_name.to_string()))?;

        if !provider.supports_feature(feature_name) {
            return Err(ProviderError::FeatureNotSupported {
                provider: provider_name.to_string(),
                feature: feature_name.to_string(),
            });
        }

        let operations = provider.feature_operations(feature_name)?;
        let routes = operations
            .into_iter()
            .map(|operation| {
                Route::new(
                    provider_name.to_string(),
                    feature_name.to_string(),
                    operation,
                )
            })
            .collect();

        Ok(routes)
    }

    /// Find routes by pattern matching
    pub async fn find_routes_by_pattern(&self, pattern: &RoutePattern) -> ProviderResult<Vec<Route>> {
        let all_routes = self.list_all_routes().await?;

        let filtered_routes = all_routes
            .into_iter()
            .filter(|route| pattern.matches(route))
            .collect();

        Ok(filtered_routes)
    }

    /// Suggest similar routes when a route is not found
    pub async fn suggest_similar_routes(&self, invalid_route: &Route) -> ProviderResult<Vec<Route>> {
        let all_routes = self.list_all_routes().await?;

        // Find routes with similar provider names
        let mut suggestions = Vec::new();

        // Exact provider match, different feature/operation
        for route in &all_routes {
            if route.provider == invalid_route.provider {
                suggestions.push(route.clone());
            }
        }

        // Similar provider names (basic fuzzy matching)
        if suggestions.is_empty() {
            for route in &all_routes {
                if route.provider.contains(&invalid_route.provider) 
                   || invalid_route.provider.contains(&route.provider) {
                    suggestions.push(route.clone());
                }
            }
        }

        // Limit to top 5 suggestions
        suggestions.truncate(5);
        Ok(suggestions)
    }
}

/// A resolved route with validated components
#[derive(Debug, Clone)]
pub struct ResolvedRoute {
    pub route: Route,
    pub provider: String,
    pub feature: String,
    pub operation: String,
    pub available_operations: Vec<String>,
}

/// Pattern for matching routes
#[derive(Debug, Clone)]
pub struct RoutePattern {
    pub provider: Option<String>,
    pub feature: Option<String>,
    pub operation: Option<String>,
}

impl RoutePattern {
    /// Create a new route pattern
    pub fn new() -> Self {
        Self {
            provider: None,
            feature: None,
            operation: None,
        }
    }

    /// Set provider pattern
    pub fn with_provider<S: Into<String>>(mut self, provider: S) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Set feature pattern
    pub fn with_feature<S: Into<String>>(mut self, feature: S) -> Self {
        self.feature = Some(feature.into());
        self
    }

    /// Set operation pattern
    pub fn with_operation<S: Into<String>>(mut self, operation: S) -> Self {
        self.operation = Some(operation.into());
        self
    }

    /// Check if a route matches this pattern
    pub fn matches(&self, route: &Route) -> bool {
        if let Some(ref provider_pattern) = self.provider {
            if !self.pattern_matches(provider_pattern, &route.provider) {
                return false;
            }
        }

        if let Some(ref feature_pattern) = self.feature {
            if !self.pattern_matches(feature_pattern, &route.feature) {
                return false;
            }
        }

        if let Some(ref operation_pattern) = self.operation {
            if !self.pattern_matches(operation_pattern, &route.operation) {
                return false;
            }
        }

        true
    }

    /// Simple pattern matching (supports * wildcard)
    fn pattern_matches(&self, pattern: &str, value: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') {
            // Simple wildcard matching
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return value.starts_with(prefix) && value.ends_with(suffix);
            }
        }

        pattern == value
    }
}

impl Default for RoutePattern {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_pattern_matching() {
        let route = Route::new(
            "virtualbox".to_string(),
            "vm-management".to_string(),
            "start-vm".to_string(),
        );

        // Exact match
        let pattern = RoutePattern::new()
            .with_provider("virtualbox")
            .with_feature("vm-management")
            .with_operation("start-vm");
        assert!(pattern.matches(&route));

        // Wildcard provider
        let pattern = RoutePattern::new()
            .with_provider("*")
            .with_feature("vm-management");
        assert!(pattern.matches(&route));

        // Partial wildcard
        let pattern = RoutePattern::new()
            .with_provider("virtual*");
        assert!(pattern.matches(&route));

        // No match
        let pattern = RoutePattern::new()
            .with_provider("aws");
        assert!(!pattern.matches(&route));
    }
}
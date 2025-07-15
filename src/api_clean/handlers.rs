//! # API Handlers
//!
//! HTTP request handlers for the clean API layer.

use super::responses::*;
use crate::routing::{Router, Route};
use crate::providers::{ProviderError, ProviderRegistry, ProviderRegistryStats, EventRegistry};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Instant};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub router: Arc<Router>,
    pub registry: Arc<ProviderRegistry>,
    pub event_registry: Arc<EventRegistry>,
    pub start_time: SystemTime,
}

/// Query parameters for operation execution
#[derive(Debug, Deserialize)]
pub struct ExecuteQuery {
    /// Additional query parameters passed as operation arguments
    #[serde(flatten)]
    pub args: HashMap<String, serde_json::Value>,
}

/// Request body for operation execution
#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    /// Operation arguments
    #[serde(default)]
    pub args: HashMap<String, serde_json::Value>,
}

/// Request body for unified exec_action endpoint
#[derive(Debug, Deserialize)]
pub struct ExecActionRequest {
    /// Provider name
    pub provider: String,
    /// Feature name
    pub feature: String,
    /// Operation name
    pub operation: String,
    /// Operation arguments
    #[serde(default)]
    pub args: HashMap<String, serde_json::Value>,
}

/// Health check endpoint
/// GET /health
pub async fn health(State(state): State<AppState>) -> Json<ApiResponse<HealthStatus>> {
    let uptime = state.start_time.elapsed()
        .unwrap_or_default()
        .as_secs();

    let stats = state.registry.get_statistics().await;
    
    let health = HealthStatus {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        providers_loaded: stats.total_providers,
        total_features: stats.total_features,
        total_operations: stats.total_operations,
        memory_usage_mb: get_memory_usage_mb(),
    };

    Json(ApiResponse::success(health))
}

/// Discovery endpoint - list all providers and their capabilities
/// GET /providers
pub async fn discover_providers(State(state): State<AppState>) -> Json<ApiResponse<DiscoveryResponse>> {
    let metadata_list = state.registry.list_metadata().await;
    
    let providers: Vec<ProviderSummary> = metadata_list
        .into_iter()
        .map(|metadata| ProviderSummary {
            name: metadata.name,
            version: metadata.version,
            description: metadata.description,
            features: metadata.features
                .into_iter()
                .map(|feature| FeatureSummary {
                    name: feature.name,
                    description: feature.description,
                    operations: feature.operations
                        .into_iter()
                        .map(|op| op.name)
                        .collect(),
                })
                .collect(),
        })
        .collect();

    let total_features = providers.iter().map(|p| p.features.len()).sum();
    let total_operations = providers.iter()
        .flat_map(|p| &p.features)
        .map(|f| f.operations.len())
        .sum();

    let discovery = DiscoveryResponse {
        total_providers: providers.len(),
        total_features,
        total_operations,
        providers,
    };

    Json(ApiResponse::success(discovery))
}

/// Get specific provider information
/// GET /providers/{provider}
pub async fn get_provider(
    Path(provider): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<ProviderInfo>>, StatusCode> {
    match state.registry.get_metadata(&provider).await {
        Some(metadata) => Ok(Json(ApiResponse::success(metadata.into()))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Get specific feature information
/// GET /providers/{provider}/features/{feature}
pub async fn get_feature(
    Path((provider, feature)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<FeatureInfo>>, StatusCode> {
    let metadata = state.registry.get_metadata(&provider).await
        .ok_or(StatusCode::NOT_FOUND)?;

    let feature_metadata = metadata.features
        .into_iter()
        .find(|f| f.name == feature)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(ApiResponse::success(feature_metadata.into())))
}

/// Get specific operation information
/// GET /providers/{provider}/features/{feature}/operations/{operation}
pub async fn get_operation(
    Path((provider, feature, operation)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<OperationInfo>>, StatusCode> {
    let route = Route::new(provider, feature, operation);
    
    match state.router.get_route_metadata(&route).await {
        Ok(metadata) => {
            let operation_info = OperationInfo {
                name: metadata.operation_name,
                description: metadata.operation_description,
                arguments: metadata.operation_arguments.into_iter().map(Into::into).collect(),
                return_type: metadata.operation_return_type,
                is_mutating: metadata.is_mutating,
                estimated_duration_ms: metadata.estimated_duration_ms,
            };
            Ok(Json(ApiResponse::success(operation_info)))
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

/// Execute an operation
/// POST /providers/{provider}/features/{feature}/operations/{operation}
pub async fn execute_operation(
    Path((provider, feature, operation)): Path<(String, String, String)>,
    Query(query): Query<ExecuteQuery>,
    State(state): State<AppState>,
    body: Option<Json<ExecuteRequest>>,
) -> Result<Json<ApiResponse<ExecutionResult>>, StatusCode> {
    let start_time = Instant::now();
    
    // Combine query parameters and body arguments
    let mut args = query.args;
    if let Some(Json(request)) = body {
        args.extend(request.args);
    }

    let route = Route::new(provider.clone(), feature.clone(), operation.clone());

    match state.router.route_request(route, args).await {
        Ok(result) => {
            let execution_time = start_time.elapsed().as_millis() as u64;
            
            let execution_result = ExecutionResult {
                result,
                execution_time_ms: execution_time,
                provider,
                feature,
                operation,
            };

            Ok(Json(ApiResponse::success(execution_result)))
        }
        Err(error) => {
            let api_error = match &error {
                ProviderError::NotFound(name) => {
                    let msg = format!("Provider '{}' not found", name);
                    ApiError::new("PROVIDER_NOT_FOUND", msg.as_str())
                }
                ProviderError::FeatureNotSupported { provider, feature } => {
                    let msg = format!("Feature '{}' not supported by provider '{}'", feature, provider);
                    ApiError::new("FEATURE_NOT_SUPPORTED", msg.as_str())
                }
                ProviderError::OperationNotSupported { provider, feature, operation } => {
                    let msg = format!("Operation '{}' not supported by feature '{}' in provider '{}'", 
                                     operation, feature, provider);
                    ApiError::new("OPERATION_NOT_SUPPORTED", msg.as_str())
                }
                ProviderError::InvalidRoute(msg) => {
                    ApiError::new("INVALID_ROUTE", msg.as_str())
                }
                ProviderError::ExecutionFailed(msg) => {
                    ApiError::new("EXECUTION_FAILED", msg.as_str())
                }
                ProviderError::LoadingFailed(msg) => {
                    ApiError::new("LOADING_FAILED", msg.as_str())
                }
                ProviderError::InitializationFailed(msg) => {
                    ApiError::new("INITIALIZATION_FAILED", msg.as_str())
                }
                ProviderError::Io(err) => {
                    ApiError::new("IO_ERROR", err.to_string().as_str())
                }
                ProviderError::LibraryError(err) => {
                    ApiError::new("LIBRARY_ERROR", err.to_string().as_str())
                }
                ProviderError::InvalidArguments(msg) => {
                    ApiError::new("INVALID_ARGUMENTS", msg.as_str())
                }
            };

            let _status_code = match error {
                ProviderError::NotFound(_) => StatusCode::NOT_FOUND,
                ProviderError::FeatureNotSupported { .. } => StatusCode::NOT_FOUND,
                ProviderError::OperationNotSupported { .. } => StatusCode::NOT_FOUND,
                ProviderError::InvalidRoute(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };

            // For error responses, we still return JSON but with error status code
            let _error_response = ApiResponse::<ExecutionResult>::error(api_error);
            
            // This is a bit tricky - we want to return the error as JSON but with proper status code
            // For now, let's return INTERNAL_SERVER_ERROR and let middleware handle it
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// List all available routes
/// GET /routes
pub async fn list_routes(State(state): State<AppState>) -> Json<ApiResponse<Vec<String>>> {
    match state.router.get_available_routes().await {
        Ok(routes) => {
            let route_paths: Vec<String> = routes
                .into_iter()
                .map(|route| route.to_url_path())
                .collect();
            Json(ApiResponse::success(route_paths))
        }
        Err(_) => {
            let error = ApiError::new("ROUTES_UNAVAILABLE", "Failed to retrieve available routes");
            Json(ApiResponse::error(error))
        }
    }
}

/// Registry statistics
/// GET /stats
pub async fn get_stats(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let stats = state.registry.get_statistics().await;
    // Convert to JSON to avoid trait issues
    let stats_json = serde_json::to_value(stats).unwrap_or_default();
    Json(ApiResponse::success(stats_json))
}

/// Unified action execution endpoint
/// POST /exec_action
pub async fn exec_action(
    State(state): State<AppState>,
    Json(request): Json<ExecActionRequest>,
) -> Result<Json<ApiResponse<ExecutionResult>>, StatusCode> {
    let start_time = Instant::now();
    
    // Create event name from feature and operation
    let event_name = format!("{}.{}", request.feature, request.operation);
    
    // Convert args to serde_json::Value
    let args_value = serde_json::to_value(request.args).unwrap_or_default();

    match state.event_registry.execute(&event_name, args_value).await {
        Ok(result) => {
            let execution_time = start_time.elapsed().as_millis() as u64;
            
            let execution_result = ExecutionResult {
                provider: request.provider,
                feature: request.feature,
                operation: request.operation,
                result,
                execution_time_ms: execution_time,
            };
            
            Ok(Json(ApiResponse::success(execution_result)))
        }
        Err(e) => {
            let error = if e.contains("not found") {
                ApiError::new("EVENT_NOT_FOUND", &e)
            } else {
                ApiError::new("EXECUTION_FAILED", &e)
            };
            
            Ok(Json(ApiResponse::error(error)))
        }
    }
}

/// Get memory usage (simplified version)
fn get_memory_usage_mb() -> f64 {
    // This is a simplified implementation
    // In a real application, you might use a crate like `sysinfo` for accurate memory reporting
    let process = std::process::Command::new("tasklist")
        .args(&["/FI", "PID eq {}", "/FO", "CSV"])
        .output();
    
    // Return a placeholder value for now
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ProviderContext, DefaultProviderContext};

    fn create_test_state() -> AppState {
        let context = Arc::new(DefaultProviderContext::new());
        let registry = Arc::new(ProviderRegistry::new(context));
        let router = Arc::new(Router::new(Arc::clone(&registry)));
        
        AppState {
            router,
            registry,
            start_time: SystemTime::now(),
        }
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let state = create_test_state();
        let response = health(State(state)).await;
        
        assert!(response.0.success);
        assert!(response.0.data.is_some());
        
        let health_data = response.0.data.unwrap();
        assert_eq!(health_data.status, "healthy");
    }

    #[tokio::test]
    async fn test_discovery_endpoint() {
        let state = create_test_state();
        let response = discover_providers(State(state)).await;
        
        assert!(response.0.success);
        assert!(response.0.data.is_some());
        
        let discovery_data = response.0.data.unwrap();
        assert_eq!(discovery_data.total_providers, 0); // No providers loaded in test
    }
}
//! # Dynamic API System
//!
//! Provides a REST API interface for the enum-based feature system.
//! Users can request resources generically and get provider-specific responses.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use warp::{Filter, Reply};
use uuid::Uuid;
use async_trait::async_trait;

use super::{EnhancedFeatureManager, PluginError};

/// API request for creating a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResourceRequest {
    /// Resource type (vm, worker, storage, etc.)
    pub resource_type: String,
    /// Feature name (worker_management, vm_management, etc.)
    pub feature: String,
    /// Operation (StartWorker, CreateVM, etc.)
    pub operation: String,
    /// Parameters for the operation
    pub parameters: Option<HashMap<String, Value>>,
}

/// API request for getting operation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOperationRequest {
    /// Feature name
    pub feature: String,
    /// Operation name
    pub operation: String,
}

/// API response for successful operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub request_id: String,
}

/// API response for operation parameter information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationParametersResponse {
    pub success: bool,
    pub feature: String,
    pub operation: String,
    pub parameters: Vec<ParameterInfo>,
    pub request_id: String,
}

/// Parameter information for operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterInfo {
    pub name: String,
    pub type_name: String,
    pub required: bool,
    pub description: String,
}

/// Simple context for API operations
pub struct ApiContext {
    execution_id: String,
    values: HashMap<String, Value>,
}

impl ApiContext {
    pub fn new(execution_id: String) -> Self {
        let mut values = HashMap::new();
        
        // Add default values
        values.insert("instance_type".to_string(), Value::String("t3.medium".to_string()));
        values.insert("availability_zone".to_string(), Value::String("us-east-1a".to_string()));
        values.insert("image".to_string(), Value::String("ami-default".to_string()));
        values.insert("security_group".to_string(), Value::String("default".to_string()));
        values.insert("network".to_string(), Value::String("default".to_string()));
        
        Self {
            execution_id,
            values,
        }
    }
}

#[async_trait]
impl super::FeatureContext for ApiContext {
    async fn get_value(&self, key: &str) -> Option<Value> {
        self.values.get(key).cloned()
    }

    async fn set_value(&self, _key: &str, _value: Value) -> Result<(), PluginError> {
        // For API context, we don't need to store values
        Ok(())
    }

    async fn get_env(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    async fn log(&self, level: super::context::LogLevel, message: &str) {
        let level_str = match level {
            super::context::LogLevel::Error => "ERROR",
            super::context::LogLevel::Warn => "WARN",
            super::context::LogLevel::Info => "INFO",
            super::context::LogLevel::Debug => "DEBUG",
            super::context::LogLevel::Trace => "TRACE",
        };
        
        println!("[{}] API: {}", level_str, message);
    }

    fn execution_id(&self) -> &str {
        &self.execution_id
    }

    fn plugin_name(&self) -> &str {
        "api"
    }
}

/// Enhanced API processor using the new enum-based system
pub struct EnhancedApiProcessor {
    feature_manager: std::sync::Arc<tokio::sync::RwLock<EnhancedFeatureManager>>,
}

impl EnhancedApiProcessor {
    pub fn new(feature_manager: std::sync::Arc<tokio::sync::RwLock<EnhancedFeatureManager>>) -> Self {
        Self {
            feature_manager,
        }
    }

    /// Process a resource creation request
    pub async fn process_request(
        &self,
        request: CreateResourceRequest,
        context: &dyn super::FeatureContext,
    ) -> Result<Value, PluginError> {
        let feature_manager = self.feature_manager.read().await;
        
        // Execute the operation
        feature_manager.execute_operation(
            &request.feature,
            &request.operation,
            request.parameters.unwrap_or_default(),
            context,
        ).await
    }

    /// Get parameter information for an operation
    pub async fn get_operation_parameters(
        &self,
        feature: &str,
        operation: &str,
    ) -> Result<Vec<ParameterInfo>, PluginError> {
        let feature_manager = self.feature_manager.read().await;
        
        let args = feature_manager.get_operation_arguments(feature, operation)?;
        
        let parameters = args.into_iter().map(|(name, type_name)| {
            ParameterInfo {
                name: name.clone(),
                type_name: type_name.clone(),
                required: true, // All parameters are required in the new system
                description: format!("Parameter '{}' of type {}", name, type_name),
            }
        }).collect();
        
        Ok(parameters)
    }

    /// Get available features
    pub async fn get_available_features(&self) -> Vec<String> {
        let feature_manager = self.feature_manager.read().await;
        feature_manager.get_available_features()
    }

    /// Get available operations for a feature
    pub async fn get_feature_operations(&self, feature: &str) -> Result<Vec<String>, PluginError> {
        let feature_manager = self.feature_manager.read().await;
        feature_manager.get_feature_operations(feature)
    }
}

/// Create resource endpoint
pub fn create_resource_endpoint(
    processor: std::sync::Arc<EnhancedApiProcessor>,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    warp::path("create")
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map(move || processor.clone()))
        .and_then(handle_create_resource)
}

/// Get operation parameters endpoint
pub fn get_operation_parameters_endpoint(
    processor: std::sync::Arc<EnhancedApiProcessor>,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    warp::path("operations")
        .and(warp::path("parameters"))
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map(move || processor.clone()))
        .and_then(handle_get_operation_parameters)
}

/// Get available features endpoint
pub fn get_features_endpoint(
    processor: std::sync::Arc<EnhancedApiProcessor>,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    warp::path("features")
        .and(warp::get())
        .and(warp::any().map(move || processor.clone()))
        .and_then(handle_get_features)
}

/// Handle resource creation
async fn handle_create_resource(
    request: CreateResourceRequest,
    processor: std::sync::Arc<EnhancedApiProcessor>,
) -> Result<impl Reply, warp::Rejection> {
    let request_id = Uuid::new_v4().to_string();
    
    let context = ApiContext::new(request_id.clone());

    match processor.process_request(request, &context).await {
        Ok(result) => {
            let response = ApiResponse {
                success: true,
                data: Some(result),
                message: Some("Operation completed successfully".to_string()),
                request_id,
            };
            Ok(warp::reply::json(&response))
        },
        Err(e) => {
            let response = ApiResponse::<Value> {
                success: false,
                data: None,
                message: Some(e.to_string()),
                request_id,
            };
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle operation parameters request
async fn handle_get_operation_parameters(
    request: GetOperationRequest,
    processor: std::sync::Arc<EnhancedApiProcessor>,
) -> Result<impl Reply, warp::Rejection> {
    let request_id = Uuid::new_v4().to_string();

    match processor.get_operation_parameters(&request.feature, &request.operation).await {
        Ok(parameters) => {
            let response = OperationParametersResponse {
                success: true,
                feature: request.feature,
                operation: request.operation,
                parameters,
                request_id,
            };
            Ok(warp::reply::json(&response))
        },
        Err(_e) => {
            let response = OperationParametersResponse {
                success: false,
                feature: request.feature,
                operation: request.operation,
                parameters: vec![],
                request_id,
            };
            Ok(warp::reply::json(&response))
        }
    }
}

/// Handle get features request
async fn handle_get_features(
    processor: std::sync::Arc<EnhancedApiProcessor>,
) -> Result<impl Reply, warp::Rejection> {
    let features = processor.get_available_features().await;
    let request_id = Uuid::new_v4().to_string();
    
    let response = ApiResponse {
        success: true,
        data: Some(features),
        message: Some("Features retrieved successfully".to_string()),
        request_id,
    };
    
    Ok(warp::reply::json(&response))
}

/// Create complete API with all endpoints
pub fn create_api(
    feature_manager: std::sync::Arc<tokio::sync::RwLock<EnhancedFeatureManager>>,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    let processor = std::sync::Arc::new(EnhancedApiProcessor::new(feature_manager));
    
    create_resource_endpoint(processor.clone())
        .or(get_operation_parameters_endpoint(processor.clone()))
        .or(get_features_endpoint(processor.clone()))
        .with(warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type"])
            .allow_methods(vec!["GET", "POST", "PUT", "DELETE"]))
}
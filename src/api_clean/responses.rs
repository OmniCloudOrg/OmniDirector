//! # API Response Types
//!
//! Standardized response types for the API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Standard API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Success status
    pub success: bool,
    /// Response data (present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error information (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, serde_json::Value>>,
}

impl<T> ApiResponse<T> {
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: None,
        }
    }

    /// Create a successful response with metadata
    pub fn success_with_meta(data: T, meta: HashMap<String, serde_json::Value>) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: Some(meta),
        }
    }

    /// Create an error response
    pub fn error(error: ApiError) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            meta: None,
        }
    }
}

/// API error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    /// Error code
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Additional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
    /// Suggestions for resolving the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<String>>,
}

impl ApiError {
    /// Create a new API error
    pub fn new<S: Into<String>>(code: S, message: S) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            suggestions: None,
        }
    }

    /// Add details to the error
    pub fn with_details(mut self, details: HashMap<String, serde_json::Value>) -> Self {
        self.details = Some(details);
        self
    }

    /// Add suggestions to the error
    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = Some(suggestions);
        self
    }
}

/// Provider information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub license: Option<String>,
    pub features: Vec<FeatureInfo>,
    pub file_path: Option<String>,
}

/// Feature information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureInfo {
    pub name: String,
    pub description: String,
    pub version: String,
    pub operations: Vec<OperationInfo>,
}

/// Operation information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationInfo {
    pub name: String,
    pub description: String,
    pub arguments: Vec<ArgumentInfo>,
    pub return_type: String,
    pub is_mutating: bool,
    pub estimated_duration_ms: Option<u64>,
}

/// Argument information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentInfo {
    pub name: String,
    pub argument_type: String,
    pub description: String,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
}

/// Operation execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub result: serde_json::Value,
    pub execution_time_ms: u64,
    pub provider: String,
    pub feature: String,
    pub operation: String,
}

/// System health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub providers_loaded: usize,
    pub total_features: usize,
    pub total_operations: usize,
    pub memory_usage_mb: f64,
}

/// Discovery response for available routes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResponse {
    pub total_providers: usize,
    pub total_features: usize,
    pub total_operations: usize,
    pub providers: Vec<ProviderSummary>,
}

/// Provider summary for discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSummary {
    pub name: String,
    pub version: String,
    pub description: String,
    pub features: Vec<FeatureSummary>,
}

/// Feature summary for discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSummary {
    pub name: String,
    pub description: String,
    pub operations: Vec<String>,
}

/// Convert from internal types to API response types
impl From<crate::providers::ProviderMetadata> for ProviderInfo {
    fn from(metadata: crate::providers::ProviderMetadata) -> Self {
        Self {
            name: metadata.name,
            version: metadata.version,
            description: metadata.description,
            author: metadata.author,
            license: metadata.license,
            features: metadata.features.into_iter().map(Into::into).collect(),
            file_path: metadata.file_path,
        }
    }
}

impl From<crate::providers::FeatureMetadata> for FeatureInfo {
    fn from(metadata: crate::providers::FeatureMetadata) -> Self {
        Self {
            name: metadata.name,
            description: metadata.description,
            version: metadata.version,
            operations: metadata.operations.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::providers::OperationMetadata> for OperationInfo {
    fn from(metadata: crate::providers::OperationMetadata) -> Self {
        Self {
            name: metadata.name,
            description: metadata.description,
            arguments: metadata.arguments.into_iter().map(Into::into).collect(),
            return_type: metadata.return_type,
            is_mutating: metadata.is_mutating,
            estimated_duration_ms: metadata.estimated_duration_ms,
        }
    }
}

impl From<crate::providers::ArgumentMetadata> for ArgumentInfo {
    fn from(metadata: crate::providers::ArgumentMetadata) -> Self {
        Self {
            name: metadata.name,
            argument_type: format!("{:?}", metadata.arg_type), // Convert enum to string
            description: metadata.description,
            required: metadata.required,
            default_value: metadata.default_value,
        }
    }
}
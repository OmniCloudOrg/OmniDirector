//! # Provider Metadata
//!
//! Metadata structures for providers and their capabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provider metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetadata {
    /// Provider name
    pub name: String,
    
    /// Provider version
    pub version: String,
    
    /// Provider description
    pub description: String,
    
    /// Provider author
    pub author: Option<String>,
    
    /// Provider license
    pub license: Option<String>,
    
    /// Supported features
    pub features: Vec<FeatureMetadata>,
    
    /// Provider settings schema
    pub settings_schema: Option<serde_json::Value>,
    
    /// File path (for loaded providers)
    pub file_path: Option<String>,
}

/// Feature metadata within a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMetadata {
    /// Feature name
    pub name: String,
    
    /// Feature description
    pub description: String,
    
    /// Feature version
    pub version: String,
    
    /// Available operations
    pub operations: Vec<OperationMetadata>,
}

/// Operation metadata within a feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMetadata {
    /// Operation name
    pub name: String,
    
    /// Operation description
    pub description: String,
    
    /// Required arguments
    pub arguments: Vec<ArgumentMetadata>,
    
    /// Return type description
    pub return_type: String,
    
    /// Whether operation modifies state
    pub is_mutating: bool,
    
    /// Estimated execution time in milliseconds
    pub estimated_duration_ms: Option<u64>,
}

/// Argument metadata for operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentMetadata {
    /// Argument name
    pub name: String,
    
    /// Argument description
    pub description: String,
    
    /// Argument type
    pub arg_type: ArgumentType,
    
    /// Whether argument is required
    pub required: bool,
    
    /// Default value
    pub default_value: Option<serde_json::Value>,
    
    /// Validation constraints
    pub constraints: Option<ArgumentConstraints>,
}

/// Argument types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ArgumentType {
    String { max_length: Option<usize> },
    Number { min: Option<f64>, max: Option<f64> },
    Integer { min: Option<i64>, max: Option<i64> },
    Boolean,
    Array { item_type: Box<ArgumentType> },
    Object { properties: HashMap<String, ArgumentType> },
    Enum { values: Vec<String> },
    Any,
}

/// Validation constraints for arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentConstraints {
    /// Regex pattern for string validation
    pub pattern: Option<String>,
    
    /// Minimum value for numbers
    pub min: Option<f64>,
    
    /// Maximum value for numbers
    pub max: Option<f64>,
    
    /// Allowed values
    pub allowed_values: Option<Vec<serde_json::Value>>,
    
    /// Custom validation message
    pub message: Option<String>,
}

impl ProviderMetadata {
    /// Create new provider metadata
    pub fn new(name: String, version: String, description: String) -> Self {
        Self {
            name,
            version,
            description,
            author: None,
            license: None,
            features: Vec::new(),
            settings_schema: None,
            file_path: None,
        }
    }
    
    /// Add a feature to the provider
    pub fn add_feature(&mut self, feature: FeatureMetadata) {
        self.features.push(feature);
    }
    
    /// Get feature by name
    pub fn get_feature(&self, name: &str) -> Option<&FeatureMetadata> {
        self.features.iter().find(|f| f.name == name)
    }
    
    /// Get all feature names
    pub fn feature_names(&self) -> Vec<String> {
        self.features.iter().map(|f| f.name.clone()).collect()
    }
}

impl FeatureMetadata {
    /// Create new feature metadata
    pub fn new(name: String, description: String, version: String) -> Self {
        Self {
            name,
            description,
            version,
            operations: Vec::new(),
        }
    }
    
    /// Add an operation to the feature
    pub fn add_operation(&mut self, operation: OperationMetadata) {
        self.operations.push(operation);
    }
    
    /// Get operation by name
    pub fn get_operation(&self, name: &str) -> Option<&OperationMetadata> {
        self.operations.iter().find(|o| o.name == name)
    }
    
    /// Get all operation names
    pub fn operation_names(&self) -> Vec<String> {
        self.operations.iter().map(|o| o.name.clone()).collect()
    }
}
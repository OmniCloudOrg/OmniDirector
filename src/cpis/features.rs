//! # Generic Features System
//!
//! Manages feature definitions, schemas, and capabilities that plugins can declare.
//! Features are completely generic and not hardcoded into the core system.

use std::collections::HashMap;
use std::path::Path;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use super::PluginError;
// Legacy feature system - replaced by enum_features

/// Feature definition loaded from external sources (JSON, plugins, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDef {
    /// Feature name (completely generic)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Version of the feature schema
    pub version: String,
    /// Available actions for this feature
    pub actions: HashMap<String, ActionDef>,
    /// Global settings for this feature
    pub global_settings: Option<HashMap<String, ArgumentDef>>,
}

/// Action definition within a feature (generic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    /// Action name (completely generic)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Required arguments for this action
    pub arguments: Vec<ArgumentDef>,
    /// Expected return type
    pub return_type: ReturnType,
    /// Whether this action modifies state
    pub is_mutating: bool,
    /// Estimated execution time in milliseconds
    pub estimated_duration_ms: Option<u64>,
}

/// Argument definition for actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArgumentDef {
    pub name: String,
    pub description: String,
    pub arg_type: ArgumentType,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub constraints: Option<ArgumentConstraints>,
}

/// Argument types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ArgumentType {
    String { max_length: Option<usize> },
    Number { min: Option<f64>, max: Option<f64> },
    Boolean,
    Array { item_type: Box<ArgumentType> },
    Object { properties: HashMap<String, ArgumentType> },
    Enum { values: Vec<String> },
    Any,
}

/// Validation constraints for arguments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArgumentConstraints {
    pub pattern: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub allowed_values: Option<Vec<serde_json::Value>>,
}

/// Expected return type for actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ReturnType {
    Void,
    String,
    Number,
    Boolean,
    Object { schema: HashMap<String, ArgumentType> },
    Array { item_type: Box<ReturnType> },
    Any,
}

/// Registry for managing feature definitions
#[derive(Debug)]
pub struct FeatureRegistry {
    /// Map of feature name to feature definition
    features: RwLock<HashMap<String, FeatureDef>>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self {
            features: RwLock::new(HashMap::new()),
        }
    }

    /// Load feature schemas from a directory (generic schema loader)
    pub async fn load_schemas<P: AsRef<Path>>(&self, schemas_dir: P) -> Result<usize, PluginError> {
        let schemas_dir = schemas_dir.as_ref();
        
        if !schemas_dir.exists() {
            tokio::fs::create_dir_all(schemas_dir).await?;
            return Ok(0); // No schemas to load, no defaults created
        }

        let mut loaded_count = 0;
        let mut read_dir = tokio::fs::read_dir(schemas_dir).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_feature_schema(&path).await {
                    Ok(_) => loaded_count += 1,
                    Err(e) => eprintln!("Failed to load feature schema from {:?}: {}", path, e),
                }
            }
        }

        Ok(loaded_count)
    }

    /// Load a single feature schema from a file
    async fn load_feature_schema<P: AsRef<Path>>(&self, path: P) -> Result<(), PluginError> {
        let content = tokio::fs::read_to_string(path).await?;
        let feature: FeatureDef = serde_json::from_str(&content)?;
        
        let mut features = self.features.write().await;
        features.insert(feature.name.clone(), feature);
        
        Ok(())
    }

    /// Register a feature definition dynamically (for plugins to use)
    pub async fn register_feature(&self, feature: FeatureDef) -> Result<(), PluginError> {
        let mut features = self.features.write().await;
        if features.contains_key(&feature.name) {
            return Err(PluginError::InitializationFailed(
                format!("Feature '{}' already registered", feature.name)
            ));
        }
        features.insert(feature.name.clone(), feature);
        Ok(())
    }

    // Note: register_feature_from_trait removed as Feature trait is deprecated in favor of enum-based system

    /// Check if a feature is supported
    pub async fn is_feature_supported(&self, feature_name: &str) -> bool {
        let features = self.features.read().await;
        features.contains_key(feature_name)
    }

    /// Get list of all available features
    pub async fn list_features(&self) -> Vec<String> {
        let features = self.features.read().await;
        features.keys().cloned().collect()
    }

    /// Get feature definition
    pub async fn get_feature(&self, feature_name: &str) -> Option<FeatureDef> {
        let features = self.features.read().await;
        features.get(feature_name).cloned()
    }

    /// Get available actions for a feature
    pub async fn get_feature_actions(&self, feature_name: &str) -> Result<Vec<String>, PluginError> {
        let features = self.features.read().await;
        let feature = features.get(feature_name)
            .ok_or_else(|| PluginError::UnsupportedFeature(feature_name.to_string()))?;
        
        Ok(feature.actions.keys().cloned().collect())
    }

    /// Get arguments for a specific action
    pub async fn get_action_arguments(&self, feature_name: &str, action_name: &str) -> Result<Vec<ArgumentDef>, PluginError> {
        let features = self.features.read().await;
        let feature = features.get(feature_name)
            .ok_or_else(|| PluginError::UnsupportedFeature(feature_name.to_string()))?;
        
        let action = feature.actions.get(action_name)
            .ok_or_else(|| PluginError::InvalidArgument(format!("Action {} not found in feature {}", action_name, feature_name)))?;
        
        Ok(action.arguments.clone())
    }

    /// Validate that an action exists and arguments are valid
    pub async fn validate_action(&self, feature_name: &str, action_name: &str, args: &HashMap<String, Value>) -> Result<(), PluginError> {
        let features = self.features.read().await;
        let feature = features.get(feature_name)
            .ok_or_else(|| PluginError::UnsupportedFeature(feature_name.to_string()))?;
        
        let action = feature.actions.get(action_name)
            .ok_or_else(|| PluginError::InvalidArgument(format!("Action {} not found in feature {}", action_name, feature_name)))?;
        
        // Validate required arguments are present
        for arg_def in &action.arguments {
            if arg_def.required && !args.contains_key(&arg_def.name) {
                return Err(PluginError::InvalidArgument(format!("Missing required argument: {}", arg_def.name)));
            }
            
            // Validate argument types if present
            if let Some(value) = args.get(&arg_def.name) {
                self.validate_argument_type(value, &arg_def.arg_type, &arg_def.name)?;
            }
        }
        
        Ok(())
    }
    
    /// Validate argument type matches expected type
    fn validate_argument_type(&self, value: &Value, expected_type: &ArgumentType, arg_name: &str) -> Result<(), PluginError> {
        match (value, expected_type) {
            (Value::String(s), ArgumentType::String { max_length }) => {
                if let Some(max_len) = max_length {
                    if s.len() > *max_len {
                        return Err(PluginError::InvalidArgument(format!("String argument '{}' exceeds maximum length of {}", arg_name, max_len)));
                    }
                }
            },
            (Value::Number(n), ArgumentType::Number { min, max }) => {
                let num = n.as_f64().unwrap_or(0.0);
                if let Some(min_val) = min {
                    if num < *min_val {
                        return Err(PluginError::InvalidArgument(format!("Number argument '{}' is below minimum value of {}", arg_name, min_val)));
                    }
                }
                if let Some(max_val) = max {
                    if num > *max_val {
                        return Err(PluginError::InvalidArgument(format!("Number argument '{}' is above maximum value of {}", arg_name, max_val)));
                    }
                }
            },
            (Value::Bool(_), ArgumentType::Boolean) => {},
            (Value::Array(_), ArgumentType::Array { .. }) => {
                // Could add more detailed array validation here
            },
            (Value::Object(_), ArgumentType::Object { .. }) => {
                // Could add more detailed object validation here
            },
            _ => {
                return Err(PluginError::InvalidArgument(format!("Argument '{}' has incorrect type", arg_name)));
            }
        }
        
        Ok(())
    }
}
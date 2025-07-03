//! # Features System
//!
//! Manages feature definitions, schemas, and capabilities that plugins can declare.
//! Features are loaded from JSON files and define what actions are available.

use std::collections::HashMap;
use std::path::Path;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use super::PluginError;

/// Feature definition loaded from JSON schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDef {
    /// Feature name (e.g., "VM_Manage", "File_Storage")
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

/// Action definition within a feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    /// Action name (e.g., "create_vm", "delete_file")
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentDef {
    /// Argument name
    pub name: String,
    /// Argument description
    pub description: String,
    /// Argument type
    pub arg_type: ArgumentType,
    /// Whether this argument is required
    pub required: bool,
    /// Default value if not required
    pub default_value: Option<Value>,
    /// Validation constraints
    pub constraints: Option<ArgumentConstraints>,
}

/// Types of arguments that can be used
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ArgumentType {
    String { max_length: Option<usize> },
    Number { min: Option<f64>, max: Option<f64> },
    Boolean,
    Array { item_type: Box<ArgumentType> },
    Object { properties: HashMap<String, ArgumentType> },
    Enum { values: Vec<String> },
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
    /// Allowed values for enums
    pub allowed_values: Option<Vec<Value>>,
}

/// Expected return type for actions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ReturnType {
    Void,
    String,
    Number,
    Boolean,
    Object { schema: HashMap<String, ArgumentType> },
    Array { item_type: Box<ReturnType> },
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

    /// Load feature schemas from a directory
    pub async fn load_schemas<P: AsRef<Path>>(&self, schemas_dir: P) -> Result<usize, PluginError> {
        let schemas_dir = schemas_dir.as_ref();
        
        if !schemas_dir.exists() {
            tokio::fs::create_dir_all(schemas_dir).await?;
            self.create_default_schemas(schemas_dir).await?;
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

    /// Create default feature schemas
    async fn create_default_schemas<P: AsRef<Path>>(&self, schemas_dir: P) -> Result<(), PluginError> {
        let vm_manage_schema = FeatureDef {
            name: "VM_Manage".to_string(),
            description: "Virtual Machine management capabilities".to_string(),
            version: "1.0.0".to_string(),
            actions: HashMap::from([
                ("create_vm".to_string(), ActionDef {
                    name: "create_vm".to_string(),
                    description: "Create a new virtual machine".to_string(),
                    arguments: vec![
                        ArgumentDef {
                            name: "name".to_string(),
                            description: "VM name".to_string(),
                            arg_type: ArgumentType::String { max_length: Some(255) },
                            required: true,
                            default_value: None,
                            constraints: None,
                        },
                        ArgumentDef {
                            name: "memory_mb".to_string(),
                            description: "Memory in megabytes".to_string(),
                            arg_type: ArgumentType::Number { min: Some(512.0), max: Some(1048576.0) },
                            required: true,
                            default_value: None,
                            constraints: None,
                        },
                        ArgumentDef {
                            name: "cpu_count".to_string(),
                            description: "Number of CPUs".to_string(),
                            arg_type: ArgumentType::Number { min: Some(1.0), max: Some(64.0) },
                            required: true,
                            default_value: None,
                            constraints: None,
                        },
                    ],
                    return_type: ReturnType::Object {
                        schema: HashMap::from([
                            ("vm_id".to_string(), ArgumentType::String { max_length: None }),
                            ("status".to_string(), ArgumentType::String { max_length: None }),
                        ]),
                    },
                    is_mutating: true,
                    estimated_duration_ms: Some(30000),
                }),
                ("delete_vm".to_string(), ActionDef {
                    name: "delete_vm".to_string(),
                    description: "Delete a virtual machine".to_string(),
                    arguments: vec![
                        ArgumentDef {
                            name: "vm_id".to_string(),
                            description: "VM identifier".to_string(),
                            arg_type: ArgumentType::String { max_length: None },
                            required: true,
                            default_value: None,
                            constraints: None,
                        },
                    ],
                    return_type: ReturnType::Boolean,
                    is_mutating: true,
                    estimated_duration_ms: Some(15000),
                }),
            ]),
            global_settings: Some(HashMap::from([
                ("default_resource_pool".to_string(), ArgumentDef {
                    name: "default_resource_pool".to_string(),
                    description: "Default resource pool for VMs".to_string(),
                    arg_type: ArgumentType::String { max_length: None },
                    required: false,
                    default_value: Some(Value::String("default".to_string())),
                    constraints: None,
                }),
            ])),
        };

        let file_storage_schema = FeatureDef {
            name: "File_Storage".to_string(),
            description: "File storage management capabilities".to_string(),
            version: "1.0.0".to_string(),
            actions: HashMap::from([
                ("upload_file".to_string(), ActionDef {
                    name: "upload_file".to_string(),
                    description: "Upload a file to storage".to_string(),
                    arguments: vec![
                        ArgumentDef {
                            name: "file_path".to_string(),
                            description: "Local file path".to_string(),
                            arg_type: ArgumentType::String { max_length: None },
                            required: true,
                            default_value: None,
                            constraints: None,
                        },
                        ArgumentDef {
                            name: "destination".to_string(),
                            description: "Storage destination path".to_string(),
                            arg_type: ArgumentType::String { max_length: None },
                            required: true,
                            default_value: None,
                            constraints: None,
                        },
                    ],
                    return_type: ReturnType::Object {
                        schema: HashMap::from([
                            ("file_id".to_string(), ArgumentType::String { max_length: None }),
                            ("url".to_string(), ArgumentType::String { max_length: None }),
                        ]),
                    },
                    is_mutating: true,
                    estimated_duration_ms: Some(5000),
                }),
            ]),
            global_settings: None,
        };

        // Write schemas to files
        let vm_content = serde_json::to_string_pretty(&vm_manage_schema)?;
        let file_content = serde_json::to_string_pretty(&file_storage_schema)?;

        tokio::fs::write(schemas_dir.as_ref().join("vm_manage.json"), vm_content).await?;
        tokio::fs::write(schemas_dir.as_ref().join("file_storage.json"), file_content).await?;

        Ok(())
    }

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
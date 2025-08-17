//! # Feature Registry
//!
//! Manages feature interface definitions (not callable providers)
//! Features define the minimum API surface that CPIs must implement

use std::collections::HashMap;

/// Feature interface definition
#[derive(Debug, Clone)]
pub struct FeatureInterface {
    pub name: String,
    pub description: String,
    pub operations: Vec<FeatureOperation>,
}

/// Operation definition within a feature
#[derive(Debug, Clone)]
pub struct FeatureOperation {
    pub name: String,
    pub description: String,
    pub parameters: HashMap<String, ParameterDefinition>,
    pub returns: Option<String>,
}

/// Parameter definition for an operation
#[derive(Debug, Clone)]
pub struct ParameterDefinition {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    pub description: Option<String>,
}

/// Registry for feature interface definitions
pub struct FeatureRegistry {
    features: HashMap<String, FeatureInterface>,
}

impl FeatureRegistry {
    /// Create a new feature registry
    pub fn new() -> Self {
        Self {
            features: HashMap::new(),
        }
    }
    
    /// Register a feature interface
    pub fn register_feature(&mut self, feature: FeatureInterface) {
        self.features.insert(feature.name.clone(), feature);
    }
    
    /// Get a feature interface by name
    pub fn get_feature(&self, name: &str) -> Option<&FeatureInterface> {
        self.features.get(name)
    }
    
    /// List all registered features
    pub fn list_features(&self) -> Vec<&str> {
        self.features.keys().map(|s| s.as_str()).collect()
    }
    
    /// Validate that a provider implements a feature correctly
    pub fn validate_provider_feature(
        &self,
        provider_name: &str,
        feature_name: &str,
        provider_operations: &[String],
    ) -> Result<(), String> {
        let feature = self.get_feature(feature_name)
            .ok_or_else(|| format!("Feature '{}' not found", feature_name))?;
        
        // Check that provider implements all required operations
        for op in &feature.operations {
            if !provider_operations.contains(&op.name) {
                return Err(format!(
                    "Provider '{}' missing required operation '{}' for feature '{}'",
                    provider_name, op.name, feature_name
                ));
            }
        }
        
        Ok(())
    }
    
    /// Get operation definition for a feature
    pub fn get_operation_definition(
        &self,
        feature_name: &str,
        operation_name: &str,
    ) -> Option<&FeatureOperation> {
        self.features.get(feature_name)?
            .operations.iter()
            .find(|op| op.name == operation_name)
    }
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}
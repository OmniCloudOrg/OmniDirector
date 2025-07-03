//! # Argument Manager
//!
//! Manages dynamic arguments for plugins. Arguments are pulled from a central pool
//! and can be either global (static in plugin settings) or per-request (user input).

use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use super::{PluginError, ArgumentDef, ArgumentType};

/// Manages plugin arguments dynamically
#[derive(Debug)]
pub struct ArgumentManager {
    /// Global arguments available to all plugins
    global_args: RwLock<HashMap<String, ArgumentValue>>,
    /// Plugin-specific argument definitions
    plugin_args: RwLock<HashMap<String, HashMap<String, ArgumentDef>>>,
    /// Per-request argument values
    request_args: RwLock<HashMap<String, ArgumentValue>>,
}

/// Represents an argument value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentValue {
    /// The actual value
    pub value: Value,
    /// When this value was set
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Source of this value (global, user_input, computed, etc.)
    pub source: ArgumentSource,
    /// Whether this value is sensitive (for logging/debugging)
    pub is_sensitive: bool,
}

/// Source of an argument value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ArgumentSource {
    /// Set globally in system configuration
    Global,
    /// Provided by user input for a specific request
    UserInput,
    /// Computed by the system
    Computed,
    /// Default value from argument definition
    Default,
    /// Environment variable
    Environment,
    /// Configuration file
    ConfigFile,
}

/// Argument resolution strategy
#[derive(Debug, Clone)]
pub enum ArgumentResolution {
    /// Use global value if available, otherwise fail
    GlobalOnly,
    /// Use request value if available, otherwise global, otherwise fail
    RequestThenGlobal,
    /// Use global value if available, otherwise request, otherwise fail
    GlobalThenRequest,
    /// Use default value from definition if no other source available
    UseDefault,
}

impl ArgumentManager {
    pub fn new() -> Self {
        Self {
            global_args: RwLock::new(HashMap::new()),
            plugin_args: RwLock::new(HashMap::new()),
            request_args: RwLock::new(HashMap::new()),
        }
    }

    /// Register argument definitions for a plugin
    pub async fn register_plugin_arguments(
        &self,
        plugin_name: &str,
        arguments: Vec<ArgumentDef>,
    ) -> Result<(), PluginError> {
        let mut plugin_args = self.plugin_args.write().await;
        let plugin_arg_map = plugin_args.entry(plugin_name.to_string()).or_insert_with(HashMap::new);
        
        for arg_def in arguments {
            plugin_arg_map.insert(arg_def.name.clone(), arg_def);
        }
        
        Ok(())
    }

    /// Set a global argument value
    pub async fn set_global_argument(
        &self,
        name: &str,
        value: Value,
        is_sensitive: bool,
    ) -> Result<(), PluginError> {
        let arg_value = ArgumentValue {
            value,
            timestamp: chrono::Utc::now(),
            source: ArgumentSource::Global,
            is_sensitive,
        };
        
        let mut global_args = self.global_args.write().await;
        global_args.insert(name.to_string(), arg_value);
        
        Ok(())
    }

    /// Set a request-specific argument value
    pub async fn set_request_argument(
        &self,
        request_id: &str,
        name: &str,
        value: Value,
    ) -> Result<(), PluginError> {
        let arg_value = ArgumentValue {
            value,
            timestamp: chrono::Utc::now(),
            source: ArgumentSource::UserInput,
            is_sensitive: false,
        };
        
        let request_key = format!("{}:{}", request_id, name);
        let mut request_args = self.request_args.write().await;
        request_args.insert(request_key, arg_value);
        
        Ok(())
    }

    /// Get an argument value using the specified resolution strategy
    pub async fn get_argument(
        &self,
        plugin_name: &str,
        arg_name: &str,
        request_id: Option<&str>,
        resolution: ArgumentResolution,
    ) -> Result<ArgumentValue, PluginError> {
        match resolution {
            ArgumentResolution::GlobalOnly => {
                self.get_global_argument(arg_name).await
            },
            ArgumentResolution::RequestThenGlobal => {
                if let Some(req_id) = request_id {
                    if let Ok(value) = self.get_request_argument(req_id, arg_name).await {
                        return Ok(value);
                    }
                }
                self.get_global_argument(arg_name).await
            },
            ArgumentResolution::GlobalThenRequest => {
                if let Ok(value) = self.get_global_argument(arg_name).await {
                    return Ok(value);
                }
                if let Some(req_id) = request_id {
                    self.get_request_argument(req_id, arg_name).await
                } else {
                    Err(PluginError::InvalidArgument(format!("Argument '{}' not found", arg_name)))
                }
            },
            ArgumentResolution::UseDefault => {
                // Try other sources first
                if let Some(req_id) = request_id {
                    if let Ok(value) = self.get_request_argument(req_id, arg_name).await {
                        return Ok(value);
                    }
                }
                if let Ok(value) = self.get_global_argument(arg_name).await {
                    return Ok(value);
                }
                // Fall back to default value
                self.get_default_argument(plugin_name, arg_name).await
            },
        }
    }

    /// Get a global argument value
    async fn get_global_argument(&self, name: &str) -> Result<ArgumentValue, PluginError> {
        let global_args = self.global_args.read().await;
        global_args.get(name)
            .cloned()
            .ok_or_else(|| PluginError::InvalidArgument(format!("Global argument '{}' not found", name)))
    }

    /// Get a request-specific argument value
    async fn get_request_argument(&self, request_id: &str, name: &str) -> Result<ArgumentValue, PluginError> {
        let request_key = format!("{}:{}", request_id, name);
        let request_args = self.request_args.read().await;
        request_args.get(&request_key)
            .cloned()
            .ok_or_else(|| PluginError::InvalidArgument(format!("Request argument '{}' not found", name)))
    }

    /// Get default value for an argument
    async fn get_default_argument(&self, plugin_name: &str, arg_name: &str) -> Result<ArgumentValue, PluginError> {
        let plugin_args = self.plugin_args.read().await;
        let plugin_arg_map = plugin_args.get(plugin_name)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;
        
        let arg_def = plugin_arg_map.get(arg_name)
            .ok_or_else(|| PluginError::InvalidArgument(format!("Argument definition '{}' not found for plugin '{}'", arg_name, plugin_name)))?;
        
        if let Some(default_value) = &arg_def.default_value {
            Ok(ArgumentValue {
                value: default_value.clone(),
                timestamp: chrono::Utc::now(),
                source: ArgumentSource::Default,
                is_sensitive: false,
            })
        } else {
            Err(PluginError::InvalidArgument(format!("No default value for argument '{}'", arg_name)))
        }
    }

    /// Resolve all arguments for a plugin action
    pub async fn resolve_action_arguments(
        &self,
        plugin_name: &str,
        action_args: &[ArgumentDef],
        user_args: &HashMap<String, Value>,
        request_id: Option<&str>,
    ) -> Result<HashMap<String, ArgumentValue>, PluginError> {
        let mut resolved_args = HashMap::new();
        
        for arg_def in action_args {
            let arg_value = if user_args.contains_key(&arg_def.name) {
                // User provided this argument
                ArgumentValue {
                    value: user_args[&arg_def.name].clone(),
                    timestamp: chrono::Utc::now(),
                    source: ArgumentSource::UserInput,
                    is_sensitive: false,
                }
            } else {
                // Try to resolve from other sources
                self.get_argument(
                    plugin_name,
                    &arg_def.name,
                    request_id,
                    ArgumentResolution::UseDefault,
                ).await?
            };
            
            // Validate the argument type
            self.validate_argument_value(&arg_value.value, &arg_def.arg_type, &arg_def.name)?;
            
            resolved_args.insert(arg_def.name.clone(), arg_value);
        }
        
        Ok(resolved_args)
    }

    /// Validate that a value matches the expected argument type
    fn validate_argument_value(
        &self,
        value: &Value,
        expected_type: &ArgumentType,
        arg_name: &str,
    ) -> Result<(), PluginError> {
        match (value, expected_type) {
            (Value::String(s), ArgumentType::String { max_length }) => {
                if let Some(max_len) = max_length {
                    if s.len() > *max_len {
                        return Err(PluginError::InvalidArgument(
                            format!("String argument '{}' exceeds maximum length of {}", arg_name, max_len)
                        ));
                    }
                }
            },
            (Value::Number(n), ArgumentType::Number { min, max }) => {
                let num = n.as_f64().unwrap_or(0.0);
                if let Some(min_val) = min {
                    if num < *min_val {
                        return Err(PluginError::InvalidArgument(
                            format!("Number argument '{}' is below minimum value of {}", arg_name, min_val)
                        ));
                    }
                }
                if let Some(max_val) = max {
                    if num > *max_val {
                        return Err(PluginError::InvalidArgument(
                            format!("Number argument '{}' is above maximum value of {}", arg_name, max_val)
                        ));
                    }
                }
            },
            (Value::Bool(_), ArgumentType::Boolean) => {},
            (Value::Array(_), ArgumentType::Array { .. }) => {
                // Could add more detailed validation here
            },
            (Value::Object(_), ArgumentType::Object { .. }) => {
                // Could add more detailed validation here
            },
            (Value::String(s), ArgumentType::Enum { values }) => {
                if !values.contains(s) {
                    return Err(PluginError::InvalidArgument(
                        format!("Enum argument '{}' has invalid value '{}', allowed values: {:?}", arg_name, s, values)
                    ));
                }
            },
            _ => {
                return Err(PluginError::InvalidArgument(
                    format!("Argument '{}' has incorrect type", arg_name)
                ));
            }
        }
        
        Ok(())
    }

    /// Load global arguments from environment variables
    pub async fn load_from_environment(&self, prefix: &str) -> Result<usize, PluginError> {
        let mut loaded_count = 0;
        
        for (key, value) in std::env::vars() {
            if key.starts_with(prefix) {
                let arg_name = key.strip_prefix(prefix).unwrap_or(&key);
                
                // Try to parse as JSON first, then fall back to string
                let parsed_value = serde_json::from_str::<Value>(&value)
                    .unwrap_or_else(|_| Value::String(value));
                
                let arg_value = ArgumentValue {
                    value: parsed_value,
                    timestamp: chrono::Utc::now(),
                    source: ArgumentSource::Environment,
                    is_sensitive: arg_name.to_lowercase().contains("password") || 
                                 arg_name.to_lowercase().contains("secret") ||
                                 arg_name.to_lowercase().contains("token"),
                };
                
                let mut global_args = self.global_args.write().await;
                global_args.insert(arg_name.to_string(), arg_value);
                loaded_count += 1;
            }
        }
        
        Ok(loaded_count)
    }

    /// Get argument statistics
    pub async fn get_argument_stats(&self) -> ArgumentStats {
        let global_args = self.global_args.read().await;
        let plugin_args = self.plugin_args.read().await;
        let request_args = self.request_args.read().await;
        
        let global_count = global_args.len();
        let plugin_count = plugin_args.values().map(|args| args.len()).sum();
        let request_count = request_args.len();
        
        let sensitive_count = global_args.values()
            .filter(|arg| arg.is_sensitive)
            .count();
        
        ArgumentStats {
            global_arguments: global_count,
            plugin_arguments: plugin_count,
            request_arguments: request_count,
            sensitive_arguments: sensitive_count,
        }
    }

    /// Clear request-specific arguments older than the specified duration
    pub async fn cleanup_old_request_args(&self, max_age_hours: u32) -> Result<usize, PluginError> {
        let cutoff_time = chrono::Utc::now() - chrono::Duration::hours(max_age_hours as i64);
        let mut request_args = self.request_args.write().await;
        
        let initial_count = request_args.len();
        request_args.retain(|_, arg_value| arg_value.timestamp > cutoff_time);
        let final_count = request_args.len();
        
        Ok(initial_count - final_count)
    }

    /// Get all global argument names (excluding sensitive ones)
    pub async fn list_global_arguments(&self, include_sensitive: bool) -> Vec<String> {
        let global_args = self.global_args.read().await;
        global_args.iter()
            .filter(|(_, arg_value)| include_sensitive || !arg_value.is_sensitive)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

/// Argument manager statistics
#[derive(Debug, Clone)]
pub struct ArgumentStats {
    pub global_arguments: usize,
    pub plugin_arguments: usize,
    pub request_arguments: usize,
    pub sensitive_arguments: usize,
}
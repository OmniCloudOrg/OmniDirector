use serde::{Deserialize, Serialize};
use derive_more::Display;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs::File;
use serde_json::Value;
use log::{info, debug, error, warn, trace};
use std::io::BufReader;
use std::sync::Arc;
use std::time::Instant;
use super::error::CpiError;
use super::validator;

#[derive(Deserialize, Debug, Clone)]
pub struct Provider {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub actions: HashMap<String, ActionDef>,
    pub default_settings: Option<HashMap<String, Value>>,
}

#[derive(Deserialize, Debug, Clone, Display)]
#[display(
    "ActionDef(target={target:?}, params={params:?}, pre_exec={pre_exec:?}, post_exec={post_exec:?}, parse_rules={parse_rules:?})"
)]
pub struct ActionDef {
    pub target: ActionTarget,
    pub params: Option<Vec<String>>,
    pub pre_exec: Option<Vec<ActionDef>>,
    pub post_exec: Option<Vec<ActionDef>>,
    pub parse_rules: ParseRules,
}

#[derive(Debug, Deserialize,Serialize,Clone,PartialEq, Display)]
pub enum ActionTarget {
    #[display("Endpoint(url={url}, method={method:?}, headers={headers:?})")]
    Endpoint {
        url: String,
        method: EndPointMethod,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    },
    Command(String),
}

#[derive(Debug, Deserialize,Serialize,Clone,PartialEq, Display)]
pub enum EndPointMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Option,
    Custom(String)
    
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum ParseRules {
    #[serde(rename = "object")]
    Object {
        patterns: HashMap<String, Pattern>,
    },
    #[serde(rename = "array")]
    Array {
        separator: String,
        patterns: HashMap<String, Pattern>,
    },
    #[serde(rename = "properties")]
    Properties {
        patterns: HashMap<String, Pattern>,
        array_patterns: Option<HashMap<String, ArrayPattern>>,
        array_key: Option<String>,
        related_patterns: Option<HashMap<String, Pattern>>,
    },
}

#[derive(Deserialize, Debug, Clone)]
pub struct Pattern {
    pub regex: String,
    pub group: Option<usize>,
    pub transform: Option<String>,
    pub optional: Option<bool>,
    pub match_value: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ArrayPattern {
    pub prefix: String,
    pub index: String,
    pub object: HashMap<String, Pattern>,
}

impl Provider {
    /// Get an action definition by name
    pub fn get_action(&self, action_name: &str) -> Result<&ActionDef, CpiError> {
        trace!("Looking up action '{}' in provider '{}'", action_name, self.name);
        self.actions.get(action_name)
            .ok_or_else(|| {
                error!("Action '{}' not found in provider '{}'", action_name, self.name);
                CpiError::ActionNotFound(action_name.to_string())
            })
    }
    
    /// Get a list of all available actions
    pub fn list_actions(&self) -> Vec<String> {
        self.actions.keys().cloned().collect()
    }
    
    /// Check if a provider has a specific action
    pub fn has_action(&self, action_name: &str) -> bool {
        self.actions.contains_key(action_name)
    }
    
    /// Create a new provider from a JSON value
    pub fn from_json(json: serde_json::Value) -> Result<Self, CpiError> {
        // Then deserialize
        let provider: Provider = serde_json::from_value(json)
            .map_err(|e| CpiError::SerdeError(e))?;
            
        Ok(provider)
    }
}

/// Load a provider from a file path
pub fn load_provider(path: PathBuf) -> Result<Provider, CpiError> {
    let start = Instant::now();
    debug!("Loading provider from file: {:?}", path);
    
    // Ensure the file exists
    if !path.exists() {
        return Err(CpiError::InvalidPath(format!("Provider file does not exist: {:?}", path)));
    }
    
    // Open the file
    let file = File::open(&path).map_err(|e| {
        error!("Failed to open provider file {:?}: {}", path, e);
        CpiError::IoError(e)
    })?;
    
    // Use BufReader for more efficient reading of large files
    let reader = BufReader::new(file);
    
    // Parse the JSON
    let json: Value = serde_json::from_reader(reader).map_err(|e| {
        error!("Failed to parse provider JSON from {:?}: {}", path, e);
        CpiError::SerdeError(e)
    })?;
    
    // Deserialize into Provider
    let provider: Provider = serde_json::from_value(json.clone()).map_err(|e| {
        error!("Failed to deserialize provider from {:?}: {}", path, e);
        CpiError::SerdeError(e)
    })?;
    
    // Log results
    let elapsed = start.elapsed();
    info!("Successfully loaded provider '{}' with {} actions in {:?}", 
        provider.name, provider.actions.len(), elapsed);
    
    trace!("Provider details: type={}, actions={}, has_default_settings={}", 
        provider.provider_type, 
        provider.actions.len(), 
        provider.default_settings.is_some());
    
    // Log available actions at debug level
    debug!("Available actions in provider '{}': {:?}", 
        provider.name, 
        provider.actions.keys().collect::<Vec<_>>());
    
    Ok(provider)
}

/// Load all providers from a directory
pub fn load_providers_from_dir(dir: &PathBuf) -> Result<Vec<Provider>, CpiError> {
    let start = Instant::now();
    info!("Loading providers from directory: {:?}", dir);
    
    // Check if the directory exists
    if !dir.exists() || !dir.is_dir() {
        return Err(CpiError::InvalidPath(format!("Invalid directory path: {:?}", dir)));
    }
    
    // Read all JSON files from the directory
    let entries = std::fs::read_dir(dir).map_err(|e| {
        error!("Failed to read directory {:?}: {}", dir, e);
        CpiError::IoError(e)
    })?;
    
    let mut providers = Vec::new();
    let mut errors = Vec::new();
    
    // Process each file
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read directory entry: {}", e);
                continue;
            }
        };
        
        let path = entry.path();
        
        // Skip non-JSON files
        if !path.is_file() || path.extension().map_or(true, |ext| ext != "json") {
            continue;
        }
        
        // Try to load the provider
        match load_provider(path.clone()) {
            Ok(provider) => {
                providers.push(provider);
            },
            Err(e) => {
                warn!("Failed to load provider from {:?}: {}", path, e);
                errors.push((path, e));
            }
        }
    }
    
    // Check if we found any providers
    if providers.is_empty() {
        if errors.is_empty() {
            return Err(CpiError::NoProvidersLoaded);
        } else {
            let error_msgs = errors.iter()
                .map(|(path, e)| format!("{:?}: {}", path, e))
                .collect::<Vec<_>>()
                .join("; ");
            
            return Err(CpiError::NoProvidersLoaded);
        }
    }
    
    let elapsed = start.elapsed();
    info!("Successfully loaded {} providers in {:?}", providers.len(), elapsed);
    
    Ok(providers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;
    
    fn create_valid_provider_json() -> serde_json::Value {
        serde_json::json!({
            "name": "test_provider",
            "type": "system",
            "actions": {
                "test_action": {
                    "command": "echo hello",
                    "parse_rules": {
                        "type": "object",
                        "patterns": {
                            "output": {
                                "regex": ".*",
                                "optional": true
                            }
                        }
                    }
                }
            }
        })
    }
    
    #[test]
    fn test_provider_from_json() {
        let json = create_valid_provider_json();
        let provider = Provider::from_json(json).unwrap();
        
        assert_eq!(provider.name, "test_provider");
        assert_eq!(provider.provider_type, "system");
        assert!(provider.has_action("test_action"));
        assert_eq!(provider.list_actions(), vec!["test_action"]);
    }
    
    #[test]
    fn test_load_provider() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_provider.json");
        
        let json = create_valid_provider_json();
        let mut file = File::create(&file_path).unwrap();
        file.write_all(serde_json::to_string_pretty(&json).unwrap().as_bytes()).unwrap();
        
        let provider = load_provider(file_path).unwrap();
        assert_eq!(provider.name, "test_provider");
        assert_eq!(provider.provider_type, "system");
    }
    
    #[test]
    fn test_load_providers_from_dir() {
        let dir = tempdir().unwrap();
        
        // Create multiple provider files
        for i in 1..=3 {
            let mut json = create_valid_provider_json();
            json["name"] = serde_json::Value::String(format!("test_provider_{}", i));
            
            let file_path = dir.path().join(format!("provider_{}.json", i));
            let mut file = File::create(&file_path).unwrap();
            file.write_all(serde_json::to_string_pretty(&json).unwrap().as_bytes()).unwrap();
        }
        
        // Also create a non-JSON file that should be ignored
        let ignored_path = dir.path().join("not_a_provider.txt");
        let mut ignored_file = File::create(&ignored_path).unwrap();
        ignored_file.write_all(b"This is not a JSON file").unwrap();
        
        let providers = load_providers_from_dir(&dir.path().to_path_buf()).unwrap();
        
        assert_eq!(providers.len(), 3);
        
        // Check that we have all the expected providers
        let names: Vec<String> = providers.iter().map(|p| p.name.clone()).collect();
        assert!(names.contains(&"test_provider_1".to_string()));
        assert!(names.contains(&"test_provider_2".to_string()));
        assert!(names.contains(&"test_provider_3".to_string()));
    }
}
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs::File;
use serde_json::Value;
use super::error::CpiError;

#[derive(Deserialize, Debug, Clone)]
pub struct Provider {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub actions: HashMap<String, ActionDef>,
    pub default_settings: Option<HashMap<String, Value>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ActionDef {
    pub command: String,
    pub params: Option<Vec<String>>,
    pub pre_exec: Option<Vec<ActionDef>>,
    pub post_exec: Option<Vec<ActionDef>>,
    pub parse_rules: ParseRules,
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
    pub fn get_action(&self, action_name: &str) -> Result<&ActionDef, CpiError> {
        self.actions.get(action_name)
            .ok_or_else(|| CpiError::ActionNotFound(action_name.to_string()))
    }
}

pub fn load_provider(path: PathBuf) -> Result<Provider, CpiError> {
    let file = File::open(&path)
        .map_err(|e| CpiError::IoError(e))?;
    
    let provider: Provider = serde_json::from_reader(file)
        .map_err(|e| CpiError::SerdeError(e))?;
    
    Ok(provider)
}
use super::prelude::*;
use super::actions::CpiAction;
use std::path::PathBuf;
use std::fs::File;
use std::process::Command;
use std::sync::RwLock;
use lazy_static::lazy_static;
use anyhow::Error;

// Define structs to parse the JSON configuration
#[derive(Deserialize, Debug, Clone)]
pub struct Cpi {
    pub name: String,
    #[serde(rename = "type")]
    pub cpi_type: String,
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

// Store loaded CPIs
lazy_static! {
    static ref LOADED_CPIS: RwLock<HashMap<String, Cpi>> = RwLock::new(HashMap::new());
}

pub fn load_cpi(path: PathBuf) -> Result<()> {
    let file = File::open(&path)
        .with_context(|| format!("Failed to open CPI file: {:?}", path))?;
    
    let cpi: Cpi = serde_json::from_reader(file)
        .with_context(|| format!("Failed to parse CPI JSON from file: {:?}", path))?;
    
    LOADED_CPIS.write().unwrap().insert(cpi.name.clone(), cpi);
    Ok(())
}

pub fn unload_cpi(name: &str) -> Result<()> {
    LOADED_CPIS.write().unwrap().remove(name);
    Ok(())
}

pub fn get_cpi(name: &str) -> Option<Cpi> {
    if let Some(cpi) = LOADED_CPIS.read().unwrap().get(name) {
        Some(cpi.clone())
    } else {
        None
    }
}

// Execute a CPI action
pub fn execute_action(cpi_name: &str, action_name: &str, params: &HashMap<String, Value>) -> Result<Value, Error> {
    let cpi = get_cpi(cpi_name)
        .ok_or_else(|| anyhow::anyhow!("CPI not found: {}", cpi_name))?;
    
    let action_def = cpi.actions.get(action_name)
        .ok_or_else(|| anyhow::anyhow!("Action not found: {}", action_name))?;
    
    // Apply default settings if available
    let mut all_params = HashMap::new();
    if let Some(defaults) = &cpi.default_settings {
        for (key, value) in defaults {
            all_params.insert(key.clone(), value.clone());
        }
    }
    
    // Apply the provided params, which override defaults
    for (key, value) in params {
        all_params.insert(key.clone(), value.clone());
    }
    
    // Check if all required params are provided
    if let Some(required_params) = &action_def.params {
        for param in required_params {
            if !all_params.contains_key(param) {
                return Err(anyhow::anyhow!("Missing required parameter: {}", param));
            }
        }
    }
    
    // Execute the action and its sub-actions
    let result = execute_sub_action(&action_def, &all_params)?;
    
    Ok(result)
}

// Helper function to fill in a command template with params
fn fill_template(template: &str, params: &HashMap<String, Value>) -> Result<String, Error> {
    let mut result = template.to_string();
    
    for (key, value) in params {
        let placeholder = format!("{{{}}}", key);
        let value_str = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => value.to_string(),
        };
        
        result = result.replace(&placeholder, &value_str);
    }
    
    Ok(result)
}

// Helper function to execute a command
fn execute_command(cmd: &str) -> Result<String, Error> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    
    if parts.is_empty() {
        return Err(anyhow::anyhow!("Empty command"));
    }
    
    let output = Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .with_context(|| format!("Failed to execute command: {}", cmd))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Command failed: {}\nError: {}", cmd, stderr));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

// Helper function to execute a sub-action
fn execute_sub_action(action_def: &ActionDef, params: &HashMap<String, Value>) -> Result<Value, Error> {
    // Check if all required params are provided
    if let Some(required_params) = &action_def.params {
        for param in required_params {
            if !params.contains_key(param) {
                return Err(anyhow::anyhow!("Missing required parameter: {}", param));
            }
        }
    }
    
    // Execute pre_exec actions if any
    if let Some(pre_actions) = &action_def.pre_exec {
        for sub_action in pre_actions {
            execute_sub_action(sub_action, params)?;
        }
    }
    
    // Execute the main command
    let cmd = fill_template(&action_def.command, params)?;
    let output = execute_command(&cmd)?;
    
    // Parse the output according to the parse rules
    let result = parse_output(&output, &action_def.parse_rules, params)?;
    
    // Execute post_exec actions if any
    if let Some(post_actions) = &action_def.post_exec {
        for sub_action in post_actions {
            execute_sub_action(sub_action, params)?;
        }
    }
    
    Ok(result)
}

// Function to parse command output based on parse rules
fn parse_output(output: &str, parse_rules: &ParseRules, params: &HashMap<String, Value>) -> Result<Value, Error> {
    match parse_rules {
        ParseRules::Object { patterns } => {
            let mut result = serde_json::Map::new();
            
            for (key, pattern) in patterns {
                if let Some(value) = apply_pattern(output, pattern, params)? {
                    result.insert(key.clone(), value);
                }
            }
            
            Ok(Value::Object(result))
        },
        
        ParseRules::Array { separator, patterns } => {
            let sections = output.split(separator).filter(|s| !s.trim().is_empty());
            let mut result = Vec::new();
            
            for section in sections {
                let mut item = serde_json::Map::new();
                
                for (key, pattern) in patterns {
                    if let Some(value) = apply_pattern(section, pattern, params)? {
                        item.insert(key.clone(), value);
                    }
                }
                
                if !item.is_empty() {
                    result.push(Value::Object(item));
                }
            }
            
            Ok(Value::Array(result))
        },
        
        ParseRules::Properties { patterns, array_patterns, array_key, related_patterns } => {
            let mut result = serde_json::Map::new();
            
            // Parse regular patterns
            for (key, pattern) in patterns {
                if let Some(value) = apply_pattern(output, pattern, params)? {
                    result.insert(key.clone(), value);
                }
            }
            
            // Parse array patterns if any
            if let Some(arr_patterns) = array_patterns {
                for (key, arr_pattern) in arr_patterns {
                    let items = parse_array_pattern(output, arr_pattern)?;
                    
                    if !items.is_empty() {
                        if let Some(ak) = array_key {
                            if ak == key {
                                result.insert(key.clone(), Value::Array(items));
                            }
                        } else {
                            result.insert(key.clone(), Value::Array(items));
                        }
                    }
                }
            }
            
            // Parse related patterns if any
            if let Some(rel_patterns) = related_patterns {
                for (key, pattern) in rel_patterns {
                    if let Some(match_value) = &pattern.match_value {
                        if let Some(base_value) = result.get(match_value) {
                            if let Some(value) = apply_pattern_with_value(output, pattern, base_value, params)? {
                                result.insert(key.clone(), value);
                            }
                        }
                    } else if let Some(value) = apply_pattern(output, pattern, params)? {
                        result.insert(key.clone(), value);
                    }
                }
            }
            
            Ok(Value::Object(result))
        }
    }
}

// Helper function to apply a pattern to extract data
fn apply_pattern(text: &str, pattern: &Pattern, params: &HashMap<String, Value>) -> Result<Option<Value>, Error> {
    let regex_str = fill_template(&pattern.regex, params)?;
    let re = Regex::new(&regex_str)?;
    
    // First try line by line
    for line in text.lines() {
        if let Some(captures) = re.captures(line) {
            let group_idx = pattern.group.unwrap_or(0);
            
            if let Some(matched) = captures.get(group_idx) {
                let value_str = matched.as_str().to_string();
                let value = transform_value(&value_str, &pattern.transform)?;
                return Ok(Some(value));
            }
        }
    }
    
    // Then try the whole text as a single match
    if let Some(captures) = re.captures(text) {
        let group_idx = pattern.group.unwrap_or(0);
        
        if let Some(matched) = captures.get(group_idx) {
            let value_str = matched.as_str().to_string();
            let value = transform_value(&value_str, &pattern.transform)?;
            return Ok(Some(value));
        }
    }
    
    // If pattern is optional, return None, otherwise it's an error
    if pattern.optional.unwrap_or(false) {
        Ok(None)
    } else {
        Err(anyhow::anyhow!("Pattern not matched: {}", pattern.regex))
    }
}

// Helper function to apply a pattern with a base value
fn apply_pattern_with_value(text: &str, pattern: &Pattern, base_value: &Value, params: &HashMap<String, Value>) -> Result<Option<Value>, Error> {
    let regex_str = fill_template(&pattern.regex, params)?;
    let re = Regex::new(&regex_str)?;
    
    for line in text.lines() {
        if let Some(captures) = re.captures(line) {
            let group_idx = pattern.group.unwrap_or(0);
            
            if let Some(matched) = captures.get(group_idx) {
                let value_str = matched.as_str().to_string();
                
                // Check if it matches the base value
                if let Value::String(base_str) = base_value {
                    if &value_str == base_str {
                        return Ok(Some(Value::Bool(true)));
                    }
                }
                
                let value = transform_value(&value_str, &pattern.transform)?;
                return Ok(Some(value));
            }
        }
    }
    
    // If pattern is optional, return None, otherwise it's an error
    if pattern.optional.unwrap_or(false) {
        Ok(None)
    } else {
        Err(anyhow::anyhow!("Pattern not matched: {}", pattern.regex))
    }
}

// Helper function to parse array patterns
fn parse_array_pattern(text: &str, pattern: &ArrayPattern) -> Result<Vec<Value>, Error> {
    let mut items = Vec::new();
    let prefix_re = Regex::new(&format!("^{}({})", &pattern.prefix, &pattern.index))?;
    
    // Group lines by index
    let mut grouped_lines: HashMap<String, Vec<String>> = HashMap::new();
    
    for line in text.lines() {
        if let Some(captures) = prefix_re.captures(line) {
            if let Some(index_match) = captures.get(1) {
                let index = index_match.as_str().to_string();
                
                grouped_lines
                    .entry(index)
                    .or_insert_with(Vec::new)
                    .push(line.to_string());
            }
        }
    }
    
    // Process each group
    for (_, lines) in grouped_lines {
        let mut item = serde_json::Map::new();
        
        for (key, object_pattern) in &pattern.object {
            for line in &lines {
                if let Some(value) = apply_pattern(line, object_pattern, &HashMap::new())? {
                    item.insert(key.clone(), value);
                    break;
                }
            }
        }
        
        if !item.is_empty() {
            items.push(Value::Object(item));
        }
    }
    
    Ok(items)
}

// Helper function to transform a string value based on the transform rule
fn transform_value(value_str: &str, transform: &Option<String>) -> Result<Value, Error> {
    match transform.as_deref() {
        Some("boolean") => {
            // For boolean transform, any non-empty string becomes true
            Ok(Value::Bool(!value_str.is_empty()))
        },
        Some("number") => {
            // Parse as a number
            let num = value_str.parse::<f64>()
                .with_context(|| format!("Failed to parse number: {}", value_str))?;
            
            if let Some(num_value) = serde_json::Number::from_f64(num) {
                Ok(Value::Number(num_value))
            } else {
                Err(anyhow::anyhow!("Failed to convert to JSON number: {}", num))
            }
        },
        Some(other) => Err(anyhow::anyhow!("Unknown transform type: {}", other)),
        None => Ok(Value::String(value_str.to_string())),
    }
}
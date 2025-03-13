use super::provider::{ParseRules, Pattern, ArrayPattern};
use super::error::CpiError;
use regex::Regex;
use std::collections::HashMap;
use serde_json::{Value, Map, Number};

// Main function to parse command output based on parse rules
pub fn parse_output(output: &str, parse_rules: &ParseRules, params: &HashMap<String, Value>) -> Result<Value, CpiError> {
    match parse_rules {
        ParseRules::Object { patterns } => {
            let mut result = Map::new();
            
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
                let mut item = Map::new();
                
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
            let mut result = Map::new();
            
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
fn apply_pattern(text: &str, pattern: &Pattern, params: &HashMap<String, Value>) -> Result<Option<Value>, CpiError> {
    let regex_str = fill_template(&pattern.regex, params)?;
    let re = Regex::new(&regex_str)
        .map_err(|e| CpiError::ParseError(format!("Invalid regex '{}': {}", regex_str, e)))?;
    
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
        Err(CpiError::ParseError(format!("Pattern not matched: {}", pattern.regex)))
    }
}

// Helper function to fill in a pattern with params
fn fill_template(template: &str, params: &HashMap<String, Value>) -> Result<String, CpiError> {
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

// Helper function to apply a pattern with a base value
fn apply_pattern_with_value(text: &str, pattern: &Pattern, base_value: &Value, params: &HashMap<String, Value>) -> Result<Option<Value>, CpiError> {
    let regex_str = fill_template(&pattern.regex, params)?;
    let re = Regex::new(&regex_str)
        .map_err(|e| CpiError::ParseError(format!("Invalid regex '{}': {}", regex_str, e)))?;
    
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
        Err(CpiError::ParseError(format!("Pattern not matched: {}", pattern.regex)))
    }
}

// Helper function to parse array patterns
fn parse_array_pattern(text: &str, pattern: &ArrayPattern) -> Result<Vec<Value>, CpiError> {
    let mut items = Vec::new();
    let prefix_re = Regex::new(&format!("^{}({})", &pattern.prefix, &pattern.index))
        .map_err(|e| CpiError::ParseError(format!("Invalid regex: {}", e)))?;
    
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
        let mut item = Map::new();
        
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
fn transform_value(value_str: &str, transform: &Option<String>) -> Result<Value, CpiError> {
    match transform.as_deref() {
        Some("boolean") => {
            // For boolean transform, any non-empty string becomes true
            Ok(Value::Bool(!value_str.is_empty()))
        },
        Some("number") => {
            // Parse as a number
            let num = value_str.parse::<f64>()
                .map_err(|e| CpiError::ParseError(format!("Failed to parse number '{}': {}", value_str, e)))?;
            
            if let Some(num_value) = Number::from_f64(num) {
                Ok(Value::Number(num_value))
            } else {
                Err(CpiError::ParseError(format!("Failed to convert to JSON number: {}", num)))
            }
        },
        Some(other) => Err(CpiError::ParseError(format!("Unknown transform type: {}", other))),
        None => Ok(Value::String(value_str.to_string())),
    }
}
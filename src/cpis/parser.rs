// parser.rs - Optimized for performance
use super::provider::{ParseRules, Pattern, ArrayPattern};
use super::error::CpiError;
use crate::{debug, trace, warn};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use serde_json::{Value, Map, Number};
use once_cell::sync::Lazy;
use std::sync::Mutex;

// Cache for compiled regexes to avoid recompilation
type RegexCache = HashMap<String, Regex>;
static REGEX_CACHE: Lazy<Mutex<RegexCache>> = Lazy::new(|| Mutex::new(HashMap::new()));

// Main function to parse command output based on parse rules
pub fn parse_output(output: &str, parse_rules: &ParseRules, params: &HashMap<String, Value>) -> Result<Value, CpiError> {
    debug!("Parsing output with rule type: {:?}", parse_rule_type(parse_rules));
    
    match parse_rules {
        ParseRules::Object { patterns } => {
            debug!("Parsing object with {} patterns", patterns.len());
            let mut result = Map::new();
            
            for (key, pattern) in patterns {
                trace!("Applying pattern for key '{}' with regex: {}", key, pattern.regex);
                
                if let Some(value) = apply_pattern(output, pattern, params)? {
                    trace!("Found value for '{}': {:?}", key, value);
                    result.insert(key.clone(), value);
                } else {
                    trace!("No value found for '{}'", key);
                }
            }
            
            debug!("Finished object parsing, found {} keys", result.len());
            Ok(Value::Object(result))
        },
        
        ParseRules::Array { separator, patterns } => {
            debug!("Parsing array with separator '{}' and {} patterns", separator, patterns.len());
            
            // Pre-split the output once for better performance
            let sections: Vec<&str> = output.split(separator)
                .filter(|s| !s.trim().is_empty())
                .collect();
                
            debug!("Found {} sections to parse", sections.len());
            
            let mut result = Vec::with_capacity(sections.len());
            
            for (i, section) in sections.iter().enumerate() {
                trace!("Parsing section {} ({} bytes)", i, section.len());
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
            
            debug!("Finished array parsing, found {} items", result.len());
            Ok(Value::Array(result))
        },
        
        ParseRules::Properties { patterns, array_patterns, array_key, related_patterns } => {
            debug!("Parsing properties with {} patterns", patterns.len());
            let mut result = Map::new();
            
            // Parse regular patterns
            for (key, pattern) in patterns {
                if let Some(value) = apply_pattern(output, pattern, params)? {
                    result.insert(key.clone(), value);
                }
            }
            
            // Parse array patterns if any
            if let Some(arr_patterns) = array_patterns {
                debug!("Processing {} array patterns", arr_patterns.len());
                for (key, arr_pattern) in arr_patterns {
                    trace!("Parsing array pattern '{}'", key);
                    let items = parse_array_pattern(output, arr_pattern)?;
                    
                    if !items.is_empty() {
                        debug!("Found {} items for array pattern '{}'", items.len(), key);
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
                debug!("Processing {} related patterns", rel_patterns.len());
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
            
            debug!("Finished properties parsing, found {} keys", result.len());
            Ok(Value::Object(result))
        }
    }
}

// Helper function for cleaner logging of pattern types
fn parse_rule_type(rules: &ParseRules) -> &'static str {
    match rules {
        ParseRules::Object { .. } => "Object",
        ParseRules::Array { .. } => "Array",
        ParseRules::Properties { .. } => "Properties",
    }
}

// Helper function to get a compiled regex from cache or create a new one
fn get_or_compile_regex(pattern: &str) -> Result<Regex, CpiError> {
    // Try to get from cache first
    let mut cache = REGEX_CACHE.lock().unwrap();
    
    if let Some(regex) = cache.get(pattern) {
        return Ok(regex.clone());
    }
    
    // Compile new regex with sensible defaults for performance
    let regex = RegexBuilder::new(pattern)
        .size_limit(10 * 1024 * 1024) // 10MB limit to prevent DoS
        .dfa_size_limit(10 * 1024 * 1024)
        .build()
        .map_err(|e| CpiError::ParseError(format!("Invalid regex '{}': {}", pattern, e)))?;
    
    // Cache it for future use (limit cache size to prevent memory leaks)
    if cache.len() < 1000 {
        cache.insert(pattern.to_string(), regex.clone());
    }
    
    Ok(regex)
}

// Helper function to apply a pattern to extract data
fn apply_pattern(text: &str, pattern: &Pattern, params: &HashMap<String, Value>) -> Result<Option<Value>, CpiError> {
    let regex_str = fill_template(&pattern.regex, params)?;
    let re = get_or_compile_regex(&regex_str)?;
    
    let group_idx = pattern.group.unwrap_or(0);
    
    // First try the whole text as a single match for efficiency
    if let Some(captures) = re.captures(text) {
        if let Some(matched) = captures.get(group_idx) {
            let value_str = matched.as_str().to_string();
            let value = transform_value(&value_str, &pattern.transform)?;
            return Ok(Some(value));
        }
    }
    
    // Line-by-line approach as fallback
    // Only scan line by line if the pattern contains ^ or $ anchors which indicate line-oriented matching
    if regex_str.contains('^') || regex_str.contains('$') {
        for line in text.lines() {
            if let Some(captures) = re.captures(line) {
                if let Some(matched) = captures.get(group_idx) {
                    let value_str = matched.as_str().to_string();
                    let value = transform_value(&value_str, &pattern.transform)?;
                    return Ok(Some(value));
                }
            }
        }
    }
    
    // If pattern is optional, return None, otherwise it's an error
    if pattern.optional.unwrap_or(false) {
        trace!("Optional pattern not matched: {}", pattern.regex);
        Ok(None)
    } else {
        warn!("Required pattern not matched: {}", pattern.regex);
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
    let re = get_or_compile_regex(&regex_str)?;
    
    let group_idx = pattern.group.unwrap_or(0);
    
    // Fast path: check the whole text
    if let Some(captures) = re.captures(text) {
        if let Some(matched) = captures.get(group_idx) {
            return create_pattern_value(matched.as_str(), base_value, pattern);
        }
    }
    
    // Slower path: check line by line
    if regex_str.contains('^') || regex_str.contains('$') {
        for line in text.lines() {
            if let Some(captures) = re.captures(line) {
                if let Some(matched) = captures.get(group_idx) {
                    return create_pattern_value(matched.as_str(), base_value, pattern);
                }
            }
        }
    }
    
    // Return result based on optional flag
    if pattern.optional.unwrap_or(false) {
        Ok(None)
    } else {
        Err(CpiError::ParseError(format!("Pattern not matched: {}", pattern.regex)))
    }
}

// Helper function to create a value from a matched pattern
fn create_pattern_value(value_str: &str, base_value: &Value, pattern: &Pattern) -> Result<Option<Value>, CpiError> {
    // Check if it matches the base value
    if let Value::String(base_str) = base_value {
        if value_str == base_str {
            return Ok(Some(Value::Bool(true)));
        }
    }
    
    let value = transform_value(value_str, &pattern.transform)?;
    Ok(Some(value))
}

// Helper function to parse array patterns
fn parse_array_pattern(text: &str, pattern: &ArrayPattern) -> Result<Vec<Value>, CpiError> {
    let mut items = Vec::new();
    
    // Create a more efficient regex that combines prefix and index into a single match
    let prefix_regex_str = format!("^{}({})", &pattern.prefix, &pattern.index);
    let prefix_re = get_or_compile_regex(&prefix_regex_str)?;
    
    // Group lines by index using a more efficient approach
    let mut grouped_lines: HashMap<String, Vec<&str>> = HashMap::new();
    
    for line in text.lines() {
        if let Some(captures) = prefix_re.captures(line) {
            if let Some(index_match) = captures.get(1) {
                let index = index_match.as_str().to_string();
                grouped_lines.entry(index).or_default().push(line);
            }
        }
    }
    
    debug!("Array pattern found {} groups", grouped_lines.len());
    
    // Process each group
    for (index, lines) in grouped_lines {
        trace!("Processing array group {}", index);
        let mut item = Map::new();
        
        for (key, object_pattern) in &pattern.object {
            // Join the lines once for efficiency
            let section = lines.join("\n");
            
            if let Some(value) = apply_pattern(&section, object_pattern, &HashMap::new())? {
                item.insert(key.clone(), value);
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
            // Boolean transform: true for non-empty strings, but also handle "true"/"false" strings
            let lower = value_str.to_lowercase();
            if lower == "false" || lower == "no" || lower == "0" || value_str.is_empty() {
                Ok(Value::Bool(false))
            } else {
                Ok(Value::Bool(true))
            }
        },
        Some("number") => {
            // Parse as a number
            let num = value_str.parse::<f64>()
                .map_err(|e| CpiError::ParseError(format!("Failed to parse number '{}': {}", value_str, e)))?;
            
            // Convert to a JSON Number
            if let Some(num_value) = Number::from_f64(num) {
                Ok(Value::Number(num_value))
            } else {
                Err(CpiError::ParseError(format!("Failed to convert to JSON number: {}", num)))
            }
        },
        Some(other) => {
            warn!("Unknown transform type: {}", other);
            Err(CpiError::ParseError(format!("Unknown transform type: {}", other)))
        },
        None => Ok(Value::String(value_str.to_string())),
    }
}
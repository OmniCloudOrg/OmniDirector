// validator.rs - With enhanced error logging
use serde_json::Value;
use super::error::CpiError;
use log::{info, debug, error, warn, trace};
use std::path::Path;

// Validate CPI JSON format with detailed logging
pub fn validate_cpi_format(json: &Value, file_path: Option<&Path>) -> Result<(), CpiError> {
    let context = match file_path {
        Some(path) => format!("in file {:?}", path.display()),
        None => "in provided JSON".to_string()
    };

    // Check if it's an object
    if !json.is_object() {
        let err_msg = format!("Root element must be an object {}", context);
        error!("Validation failed: {}", err_msg);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // Required fields
    let required_fields = ["name", "type", "actions"];
    for field in required_fields.iter() {
        if json.get(*field).is_none() {
            let err_msg = format!("Missing required field: '{}' {}", field, context);
            error!("Validation failed: {}", err_msg);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
    }
    
    // Validate name
    let name = json.get("name").unwrap();
    if !name.is_string() {
        let err_msg = format!("'name' must be a string {}", context);
        error!("Validation failed: {}", err_msg);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // Validate type
    let provider_type = json.get("type").unwrap();
    if !provider_type.is_string() {
        let err_msg = format!("'type' must be a string {}", context);
        error!("Validation failed: {}", err_msg);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    let type_str = provider_type.as_str().unwrap();
    debug!("Provider type: {} {}", type_str, context);
    
    // Optional validation for type values
    if !["command", "virt", "cloud", "container"].contains(&type_str) {
        warn!("Provider type '{}' is not one of the recommended types (command, virt, cloud, container) {}", 
              type_str, context);
    }
    
    // Validate actions
    let actions = json.get("actions").unwrap();
    if !actions.is_object() {
        let err_msg = format!("'actions' must be an object {}", context);
        error!("Validation failed: {}", err_msg);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // Validate each action
    let actions_obj = actions.as_object().unwrap();
    for (action_name, action_def) in actions_obj {
        if let Err(e) = validate_action(action_name, action_def, context.as_str()) {
            error!("Action '{}' validation failed: {} {}", action_name, e, context);
            return Err(e);
        }
    }
    
    // Validate default_settings if present
    if let Some(default_settings) = json.get("default_settings") {
        if !default_settings.is_object() {
            let err_msg = format!("'default_settings' must be an object {}", context);
            error!("Validation failed: {}", err_msg);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
    }
    
    debug!("CPI validation successful {}", context);
    Ok(())
}

// Validate action definition
fn validate_action(action_name: &str, action_def: &Value, context: &str) -> Result<(), CpiError> {
    if !action_def.is_object() {
        let err_msg = format!("Action '{}' must be an object", action_name);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }

    let target = action_def.get("target");
    if target.is_none() || !target.unwrap().is_object() {
        let err_msg = format!("Action '{}' must have a 'target' field", action_name);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }

    let target_obj = target.unwrap().as_object().unwrap();
    if !target_obj.contains_key("Command") && !target_obj.contains_key("Endpoint") {
        let err_msg = format!("Action '{}' 'target' must contain either 'Command' or 'Endpoint'", action_name);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // Required field: parse_rules
    let parse_rules = action_def.get("parse_rules");
    if parse_rules.is_none() || !parse_rules.unwrap().is_object() {
        let err_msg = format!("Action '{}' must have an object 'parse_rules' field", action_name);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // Validate parse_rules
    if let Err(e) = validate_parse_rules(action_name, parse_rules.unwrap(), context) {
        return Err(e);
    }
    
    // Optional field: params (array of strings)
    if let Some(params) = action_def.get("params") {
        if !params.is_array() {
            let err_msg = format!("Action '{}' has 'params' that is not an array", action_name);
            error!("Validation failed: {} {}", err_msg, context);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
        
        for (i, param) in params.as_array().unwrap().iter().enumerate() {
            if !param.is_string() {
                let err_msg = format!("Action '{}' param at index {} is not a string", action_name, i);
                error!("Validation failed: {} {}", err_msg, context);
                return Err(CpiError::InvalidCpiFormat(err_msg));
            }
        }
    }
    
    // Optional field: pre_exec (array of actions)
    if let Some(pre_exec) = action_def.get("pre_exec") {
        if let Err(e) = validate_sub_actions(action_name, "pre_exec", pre_exec, context) {
            return Err(e);
        }
    }
    
    // Optional field: post_exec (array of actions)
    if let Some(post_exec) = action_def.get("post_exec") {
        if let Err(e) = validate_sub_actions(action_name, "post_exec", post_exec, context) {
            return Err(e);
        }
    }
    
    trace!("Action '{}' validated successfully {}", action_name, context);
    Ok(())
}

// Validate sub-actions (pre_exec or post_exec)
fn validate_sub_actions(action_name: &str, field: &str, sub_actions: &Value, context: &str) -> Result<(), CpiError> {
    if !sub_actions.is_array() {
        let err_msg = format!("Action '{}' has '{}' that is not an array", action_name, field);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    for (i, sub_action) in sub_actions.as_array().unwrap().iter().enumerate() {
        if !sub_action.is_object() {
            let err_msg = format!("Action '{}' '{}' at index {} is not an object", action_name, field, i);
            error!("Validation failed: {} {}", err_msg, context);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
        
        // Each sub-action should have the same structure as a normal action
        if let Err(e) = validate_action(&format!("{}[{}]", field, i), sub_action, context) {
            return Err(e);
        }
    }
    
    Ok(())
}

// Validate parse_rules structure
fn validate_parse_rules(action_name: &str, parse_rules: &Value, context: &str) -> Result<(), CpiError> {
    // Must have a "type" field
    let rule_type = parse_rules.get("type");
    if rule_type.is_none() || !rule_type.unwrap().is_string() {
        let err_msg = format!("Action '{}' parse_rules must have a string 'type' field", action_name);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    let rule_type_str = rule_type.unwrap().as_str().unwrap();
    
    match rule_type_str {
        "object" => {
            // For object type, must have "patterns" field
            let patterns = parse_rules.get("patterns");
            if patterns.is_none() || !patterns.unwrap().is_object() {
                let err_msg = format!("Action '{}' object parse_rules must have an object 'patterns' field", action_name);
                error!("Validation failed: {} {}", err_msg, context);
                return Err(CpiError::InvalidCpiFormat(err_msg));
            }
            
            // Validate patterns
            if let Err(e) = validate_patterns(action_name, patterns.unwrap(), context) {
                return Err(e);
            }
        },
        "array" => {
            // For array type, must have "separator" and "patterns" fields
            let separator = parse_rules.get("separator");
            if separator.is_none() || !separator.unwrap().is_string() {
                let err_msg = format!("Action '{}' array parse_rules must have a string 'separator' field", action_name);
                error!("Validation failed: {} {}", err_msg, context);
                return Err(CpiError::InvalidCpiFormat(err_msg));
            }
            
            let patterns = parse_rules.get("patterns");
            if patterns.is_none() || !patterns.unwrap().is_object() {
                let err_msg = format!("Action '{}' array parse_rules must have an object 'patterns' field", action_name);
                error!("Validation failed: {} {}", err_msg, context);
                return Err(CpiError::InvalidCpiFormat(err_msg));
            }
            
            // Validate patterns
            if let Err(e) = validate_patterns(action_name, patterns.unwrap(), context) {
                return Err(e);
            }
        },
        "properties" => {
            // For properties type, must have "patterns" field
            let patterns = parse_rules.get("patterns");
            if patterns.is_none() || !patterns.unwrap().is_object() {
                let err_msg = format!("Action '{}' properties parse_rules must have an object 'patterns' field", action_name);
                error!("Validation failed: {} {}", err_msg, context);
                return Err(CpiError::InvalidCpiFormat(err_msg));
            }
            
            // Validate patterns
            if let Err(e) = validate_patterns(action_name, patterns.unwrap(), context) {
                return Err(e);
            }
            
            // Optional field: array_patterns
            if let Some(array_patterns) = parse_rules.get("array_patterns") {
                if !array_patterns.is_object() {
                    let err_msg = format!("Action '{}' properties parse_rules has 'array_patterns' that is not an object", action_name);
                    error!("Validation failed: {} {}", err_msg, context);
                    return Err(CpiError::InvalidCpiFormat(err_msg));
                }
                
                // Validate each array pattern
                for (pattern_name, pattern_def) in array_patterns.as_object().unwrap() {
                    if let Err(e) = validate_array_pattern(action_name, pattern_name, pattern_def, context) {
                        return Err(e);
                    }
                }
            }
            
            // Optional field: related_patterns
            if let Some(related_patterns) = parse_rules.get("related_patterns") {
                if !related_patterns.is_object() {
                    let err_msg = format!("Action '{}' properties parse_rules has 'related_patterns' that is not an object", action_name);
                    error!("Validation failed: {} {}", err_msg, context);
                    return Err(CpiError::InvalidCpiFormat(err_msg));
                }
                
                // Validate patterns
                if let Err(e) = validate_patterns(action_name, related_patterns, context) {
                    return Err(e);
                }
            }
        },
        _ => {
            let err_msg = format!("Unknown parse_rules type: {}", rule_type_str);
            error!("Validation failed: {} {}", err_msg, context);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
    }
    
    trace!("Parse rules for action '{}' validated successfully {}", action_name, context);
    Ok(())
}

// Validate patterns object
fn validate_patterns(action_name: &str, patterns: &Value, context: &str) -> Result<(), CpiError> {
    for (pattern_name, pattern) in patterns.as_object().unwrap() {
        if !pattern.is_object() {
            let err_msg = format!("Action '{}' pattern '{}' is not an object", action_name, pattern_name);
            error!("Validation failed: {} {}", err_msg, context);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
        
        // Each pattern must have a regex field
        let regex = pattern.get("regex");
        if regex.is_none() || !regex.unwrap().is_string() {
            let err_msg = format!("Action '{}' pattern '{}' must have a string 'regex' field", action_name, pattern_name);
            error!("Validation failed: {} {}", err_msg, context);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
        
        // Try to validate the regex syntax
        let regex_str = regex.unwrap().as_str().unwrap();
        if let Err(e) = regex::Regex::new(regex_str) {
            let err_msg = format!("Action '{}' pattern '{}' has invalid regex '{}': {}", 
                action_name, pattern_name, regex_str, e);
            error!("Validation failed: {} {}", err_msg, context);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
        
        // Optional: group (number)
        if let Some(group) = pattern.get("group") {
            if !group.is_number() {
                let err_msg = format!("Action '{}' pattern '{}' has 'group' that is not a number", action_name, pattern_name);
                error!("Validation failed: {} {}", err_msg, context);
                return Err(CpiError::InvalidCpiFormat(err_msg));
            }
        }
        
        // Optional: transform (string)
        if let Some(transform) = pattern.get("transform") {
            if !transform.is_string() {
                let err_msg = format!("Action '{}' pattern '{}' has 'transform' that is not a string", action_name, pattern_name);
                error!("Validation failed: {} {}", err_msg, context);
                return Err(CpiError::InvalidCpiFormat(err_msg));
            }
            
            // Check if transform is one of the allowed values
            let transform_str = transform.as_str().unwrap();
            match transform_str {
                "boolean" | "number" => (),
                _ => {
                    let err_msg = format!("Action '{}' pattern '{}' has unknown transform type: {}", 
                        action_name, pattern_name, transform_str);
                    error!("Validation failed: {} {}", err_msg, context);
                    return Err(CpiError::InvalidCpiFormat(err_msg));
                }
            }
        }
        
        // Optional: optional (boolean)
        if let Some(optional) = pattern.get("optional") {
            if !optional.is_boolean() {
                let err_msg = format!("Action '{}' pattern '{}' has 'optional' that is not a boolean", action_name, pattern_name);
                error!("Validation failed: {} {}", err_msg, context);
                return Err(CpiError::InvalidCpiFormat(err_msg));
            }
        }
        
        // Optional: match_value (string)
        if let Some(match_value) = pattern.get("match_value") {
            if !match_value.is_string() {
                let err_msg = format!("Action '{}' pattern '{}' has 'match_value' that is not a string", action_name, pattern_name);
                error!("Validation failed: {} {}", err_msg, context);
                return Err(CpiError::InvalidCpiFormat(err_msg));
            }
        }
    }
    
    Ok(())
}

// Validate array_pattern structure
fn validate_array_pattern(action_name: &str, pattern_name: &str, pattern: &Value, context: &str) -> Result<(), CpiError> {
    if !pattern.is_object() {
        let err_msg = format!("Action '{}' array_pattern '{}' is not an object", action_name, pattern_name);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // Required fields
    let required_fields = ["prefix", "index", "object"];
    for field in required_fields.iter() {
        if pattern.get(*field).is_none() {
            let err_msg = format!("Action '{}' array_pattern '{}' is missing required field: {}", 
                action_name, pattern_name, field);
            error!("Validation failed: {} {}", err_msg, context);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
    }
    
    // prefix must be a string
    let prefix = pattern.get("prefix").unwrap();
    if !prefix.is_string() {
        let err_msg = format!("Action '{}' array_pattern '{}' has 'prefix' that is not a string", 
            action_name, pattern_name);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // index must be a string
    let index = pattern.get("index").unwrap();
    if !index.is_string() {
        let err_msg = format!("Action '{}' array_pattern '{}' has 'index' that is not a string", 
            action_name, pattern_name);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // Try to validate the index regex
    let index_str = index.as_str().unwrap();
    if let Err(e) = regex::Regex::new(index_str) {
        let err_msg = format!("Action '{}' array_pattern '{}' has invalid index regex '{}': {}", 
            action_name, pattern_name, index_str, e);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // object must be an object of patterns
    let object = pattern.get("object").unwrap();
    if !object.is_object() {
        let err_msg = format!("Action '{}' array_pattern '{}' has 'object' that is not an object", 
            action_name, pattern_name);
        error!("Validation failed: {} {}", err_msg, context);
        return Err(CpiError::InvalidCpiFormat(err_msg));
    }
    
    // Validate the object patterns
    if let Err(e) = validate_patterns(action_name, object, context) {
        return Err(e);
    }
    
    trace!("Array pattern '{}' for action '{}' validated successfully {}", pattern_name, action_name, context);
    Ok(())
}
use serde_json::Value;
use super::error::CpiError;

// Validate CPI JSON format
pub fn validate_cpi_format(json: &Value) -> Result<(), CpiError> {
    // Check if it's an object
    if !json.is_object() {
        return Err(CpiError::InvalidCpiFormat("Root element must be an object".to_string()));
    }
    
    // Required fields
    let required_fields = ["name", "type", "actions"];
    for field in required_fields.iter() {
        if !json.get(*field).is_some() {
            return Err(CpiError::InvalidCpiFormat(format!("Missing required field: {}", field)));
        }
    }
    
    // Validate name
    let name = json.get("name").unwrap();
    if !name.is_string() {
        return Err(CpiError::InvalidCpiFormat("'name' must be a string".to_string()));
    }
    
    // Validate type
    let provider_type = json.get("type").unwrap();
    if !provider_type.is_string() {
        return Err(CpiError::InvalidCpiFormat("'type' must be a string".to_string()));
    }
    
    // Validate actions
    let actions = json.get("actions").unwrap();
    if !actions.is_object() {
        return Err(CpiError::InvalidCpiFormat("'actions' must be an object".to_string()));
    }
    
    // Validate each action
    let actions_obj = actions.as_object().unwrap();
    for (action_name, action_def) in actions_obj {
        validate_action(action_name, action_def)?;
    }
    
    // Validate default_settings if present
    if let Some(default_settings) = json.get("default_settings") {
        if !default_settings.is_object() {
            return Err(CpiError::InvalidCpiFormat("'default_settings' must be an object".to_string()));
        }
    }
    
    Ok(())
}

// Validate action definition
fn validate_action(action_name: &str, action_def: &Value) -> Result<(), CpiError> {
    if !action_def.is_object() {
        return Err(CpiError::InvalidCpiFormat(
            format!("Action '{}' must be an object", action_name)
        ));
    }
    
    // Required field: command
    let command = action_def.get("command");
    if command.is_none() || !command.unwrap().is_string() {
        return Err(CpiError::InvalidCpiFormat(
            format!("Action '{}' must have a string 'command' field", action_name)
        ));
    }
    
    // Required field: parse_rules
    let parse_rules = action_def.get("parse_rules");
    if parse_rules.is_none() || !parse_rules.unwrap().is_object() {
        return Err(CpiError::InvalidCpiFormat(
            format!("Action '{}' must have an object 'parse_rules' field", action_name)
        ));
    }
    
    // Validate parse_rules
    validate_parse_rules(action_name, parse_rules.unwrap())?;
    
    // Optional field: params (array of strings)
    if let Some(params) = action_def.get("params") {
        if !params.is_array() {
            return Err(CpiError::InvalidCpiFormat(
                format!("Action '{}' has 'params' that is not an array", action_name)
            ));
        }
        
        for (i, param) in params.as_array().unwrap().iter().enumerate() {
            if !param.is_string() {
                return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' param at index {} is not a string", action_name, i)
                ));
            }
        }
    }
    
    // Optional field: pre_exec (array of actions)
    if let Some(pre_exec) = action_def.get("pre_exec") {
        validate_sub_actions(action_name, "pre_exec", pre_exec)?;
    }
    
    // Optional field: post_exec (array of actions)
    if let Some(post_exec) = action_def.get("post_exec") {
        validate_sub_actions(action_name, "post_exec", post_exec)?;
    }
    
    Ok(())
}

// Validate sub-actions (pre_exec or post_exec)
fn validate_sub_actions(action_name: &str, field: &str, sub_actions: &Value) -> Result<(), CpiError> {
    if !sub_actions.is_array() {
        return Err(CpiError::InvalidCpiFormat(
            format!("Action '{}' has '{}' that is not an array", action_name, field)
        ));
    }
    
    for (i, sub_action) in sub_actions.as_array().unwrap().iter().enumerate() {
        if !sub_action.is_object() {
            return Err(CpiError::InvalidCpiFormat(
                format!("Action '{}' '{}' at index {} is not an object", action_name, field, i)
            ));
        }
        
        // Each sub-action should have the same structure as a normal action
        validate_action(&format!("{}[{}]", field, i), sub_action)?;
    }
    
    Ok(())
}

// Validate parse_rules structure
fn validate_parse_rules(action_name: &str, parse_rules: &Value) -> Result<(), CpiError> {
    // Must have a "type" field
    let rule_type = parse_rules.get("type");
    if rule_type.is_none() || !rule_type.unwrap().is_string() {
        return Err(CpiError::InvalidCpiFormat(
            format!("Action '{}' parse_rules must have a string 'type' field", action_name)
        ));
    }
    
    let rule_type_str = rule_type.unwrap().as_str().unwrap();
    
    match rule_type_str {
        "object" => {
            // For object type, must have "patterns" field
            let patterns = parse_rules.get("patterns");
            if patterns.is_none() || !patterns.unwrap().is_object() {
                return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' object parse_rules must have an object 'patterns' field", action_name)
                ));
            }
            
            // Validate patterns
            validate_patterns(action_name, patterns.unwrap())?;
        },
        "array" => {
            // For array type, must have "separator" and "patterns" fields
            let separator = parse_rules.get("separator");
            if separator.is_none() || !separator.unwrap().is_string() {
                return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' array parse_rules must have a string 'separator' field", action_name)
                ));
            }
            
            let patterns = parse_rules.get("patterns");
            if patterns.is_none() || !patterns.unwrap().is_object() {
                return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' array parse_rules must have an object 'patterns' field", action_name)
                ));
            }
            
            // Validate patterns
            validate_patterns(action_name, patterns.unwrap())?;
        },
        "properties" => {
            // For properties type, must have "patterns" field
            let patterns = parse_rules.get("patterns");
            if patterns.is_none() || !patterns.unwrap().is_object() {
                return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' properties parse_rules must have an object 'patterns' field", action_name)
                ));
            }
            
            // Validate patterns
            validate_patterns(action_name, patterns.unwrap())?;
            
            // Optional field: array_patterns
            if let Some(array_patterns) = parse_rules.get("array_patterns") {
                if !array_patterns.is_object() {
                    return Err(CpiError::InvalidCpiFormat(
                        format!("Action '{}' properties parse_rules has 'array_patterns' that is not an object", action_name)
                    ));
                }
                
                // Validate each array pattern
                for (pattern_name, pattern_def) in array_patterns.as_object().unwrap() {
                    validate_array_pattern(action_name, pattern_name, pattern_def)?;
                }
            }
            
            // Optional field: related_patterns
            if let Some(related_patterns) = parse_rules.get("related_patterns") {
                if !related_patterns.is_object() {
                    return Err(CpiError::InvalidCpiFormat(
                        format!("Action '{}' properties parse_rules has 'related_patterns' that is not an object", action_name)
                    ));
                }
                
                // Validate patterns
                validate_patterns(action_name, related_patterns)?;
            }
        },
        _ => {
            return Err(CpiError::InvalidCpiFormat(
                format!("Unknown parse_rules type: {}", rule_type_str)
            ));
        }
    }
    
    Ok(())
}

// Validate patterns object
fn validate_patterns(action_name: &str, patterns: &Value) -> Result<(), CpiError> {
    for (pattern_name, pattern) in patterns.as_object().unwrap() {
        if !pattern.is_object() {
            return Err(CpiError::InvalidCpiFormat(
                format!("Action '{}' pattern '{}' is not an object", action_name, pattern_name)
            ));
        }
        
        // Each pattern must have a regex field
        let regex = pattern.get("regex");
        if regex.is_none() || !regex.unwrap().is_string() {
            return Err(CpiError::InvalidCpiFormat(
                format!("Action '{}' pattern '{}' must have a string 'regex' field", action_name, pattern_name)
            ));
        }
        
        // Optional: group (number)
        if let Some(group) = pattern.get("group") {
            if !group.is_number() {
                return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' pattern '{}' has 'group' that is not a number", action_name, pattern_name)
                ));
            }
        }
        
        // Optional: transform (string)
        if let Some(transform) = pattern.get("transform") {
            if !transform.is_string() {
                return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' pattern '{}' has 'transform' that is not a string", action_name, pattern_name)
                ));
            }
            
            // Check if transform is one of the allowed values
            let transform_str = transform.as_str().unwrap();
            match transform_str {
                "boolean" | "number" => (),
                _ => return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' pattern '{}' has unknown transform type: {}", 
                        action_name, pattern_name, transform_str)
                )),
            }
        }
        
        // Optional: optional (boolean)
        if let Some(optional) = pattern.get("optional") {
            if !optional.is_boolean() {
                return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' pattern '{}' has 'optional' that is not a boolean", action_name, pattern_name)
                ));
            }
        }
        
        // Optional: match_value (string)
        if let Some(match_value) = pattern.get("match_value") {
            if !match_value.is_string() {
                return Err(CpiError::InvalidCpiFormat(
                    format!("Action '{}' pattern '{}' has 'match_value' that is not a string", action_name, pattern_name)
                ));
            }
        }
    }
    
    Ok(())
}

// Validate array_pattern structure
fn validate_array_pattern(action_name: &str, pattern_name: &str, pattern: &Value) -> Result<(), CpiError> {
    if !pattern.is_object() {
        return Err(CpiError::InvalidCpiFormat(
            format!("Action '{}' array_pattern '{}' is not an object", action_name, pattern_name)
        ));
    }
    
    // Required fields
    let required_fields = ["prefix", "index", "object"];
    for field in required_fields.iter() {
        if pattern.get(*field).is_none() {
            return Err(CpiError::InvalidCpiFormat(
                format!("Action '{}' array_pattern '{}' is missing required field: {}", 
                    action_name, pattern_name, field)
            ));
        }
    }
    
    // prefix must be a string
    let prefix = pattern.get("prefix").unwrap();
    if !prefix.is_string() {
        return Err(CpiError::InvalidCpiFormat(
            format!("Action '{}' array_pattern '{}' has 'prefix' that is not a string", 
                action_name, pattern_name)
        ));
    }
    
    // index must be a string
    let index = pattern.get("index").unwrap();
    if !index.is_string() {
        return Err(CpiError::InvalidCpiFormat(
            format!("Action '{}' array_pattern '{}' has 'index' that is not a string", 
                action_name, pattern_name)
        ));
    }
    
    // object must be an object of patterns
    let object = pattern.get("object").unwrap();
    if !object.is_object() {
        return Err(CpiError::InvalidCpiFormat(
            format!("Action '{}' array_pattern '{}' has 'object' that is not an object", 
                action_name, pattern_name)
        ));
    }
    
    // Validate the object patterns
    validate_patterns(action_name, object)?;
    
    Ok(())
}
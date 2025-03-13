use super::provider::{Provider, ActionDef, ParseRules, Pattern};
use super::error::CpiError;
use super::parser;
use std::collections::HashMap;
use std::process::Command;
use regex::Regex;
use serde_json::Value;

// Main function to execute a CPI action
pub fn execute_action(provider: &Provider, action_name: &str, params: HashMap<String, Value>) -> Result<Value, CpiError> {
    // Get the action definition
    let action_def = provider.get_action(action_name)?;
    
    // Apply default settings if available
    let mut all_params = HashMap::new();
    if let Some(defaults) = &provider.default_settings {
        for (key, value) in defaults {
            all_params.insert(key.clone(), value.clone());
        }
    }
    
    // Apply the provided params, which override defaults
    for (key, value) in params {
        all_params.insert(key, value);
    }
    
    // Validate required parameters
    validate_params(action_def, &all_params)?;
    
    // Execute the action and its sub-actions
    execute_sub_action(action_def, &all_params)
}

// Validate parameters against action requirements
fn validate_params(action_def: &ActionDef, params: &HashMap<String, Value>) -> Result<(), CpiError> {
    if let Some(required_params) = &action_def.params {
        for param in required_params {
            if !params.contains_key(param) {
                return Err(CpiError::MissingParameter(param.clone()));
            }
        }
    }
    Ok(())
}

// Execute a single action or sub-action
fn execute_sub_action(action_def: &ActionDef, params: &HashMap<String, Value>) -> Result<Value, CpiError> {
    // Execute pre-exec actions if any
    if let Some(pre_actions) = &action_def.pre_exec {
        for sub_action in pre_actions {
            validate_params(sub_action, params)?;
            execute_sub_action(sub_action, params)?;
        }
    }
    
    // Execute the main command
    let cmd = fill_template(&action_def.command, params)?;
    let output = execute_command(&cmd)?;

    println!(">>> Output: {}", output); // Debugging output
    
    // Parse the output according to the parse rules
    let result = parser::parse_output(&output, &action_def.parse_rules, params)?;

    println!(">>> Result: {:?}", result); // Debugging output
    
    // Execute post-exec actions if any
    if let Some(post_actions) = &action_def.post_exec {
        for sub_action in post_actions {
            validate_params(sub_action, params)?;
            execute_sub_action(sub_action, params)?;
        }
    }
    
    Ok(result)
}

// Helper function to fill in a command template with params
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

// Helper function to execute a command
fn execute_command(cmd: &str) -> Result<String, CpiError> {
    println!(">>> Executing command: {}", cmd); // Debugging output

    let parts: Vec<&str> = cmd.split_whitespace().collect();
    
    if parts.is_empty() {
        return Err(CpiError::ExecutionFailed("Empty command".to_string()));
    }

    println!(">>> Parsed command parts: {:?}", parts); // Debugging output
    println!(">>> Command executable: {}", parts[0]);
    if parts.len() > 1 {
        println!(">>> Command arguments: {:?}", &parts[1..]);
    } else {
        println!(">>> No command arguments");
    }

    println!(">>> Attempting to execute command...");
    let output = Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .map_err(|e| {
            println!(">>> Execution failed: {}", e);
            CpiError::ExecutionFailed(format!("Failed to execute '{}': {}", cmd, e))
        })?;

    println!(">>> Command executed with status: {}", output.status);
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(">>> Command failed with stderr: {}", stderr);
        return Err(CpiError::ExecutionFailed(format!("Command failed: {}\nError: {}", cmd, stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    println!(">>> Command succeeded with stdout ({} bytes): {}", stdout.len(), stdout); // Debugging output
    Ok(stdout)
}

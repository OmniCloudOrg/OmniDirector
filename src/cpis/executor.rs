// executor.rs - Using run_script crate for reliable shell execution
use super::provider::{Provider, ActionDef,ActionTarget};
use super::error::CpiError;
use super::parser;
use log::{info, debug, trace, warn, error};
use rayon::result;
use std::collections::HashMap;
use std::process::Command;
use std::time::Instant;
use serde_json::Value;
use run_script::ScriptOptions;

// Main function to execute a CPI action
pub fn execute_action(provider: &Provider, action_name: &str, params: HashMap<String, Value>) -> Result<Value, CpiError> {
    let start = Instant::now();
    debug!("Executing action '{}' from provider '{}'", action_name, provider.name);
    
    // Get the action definition
    let action_def = provider.get_action(action_name)?;
    
    // Merge default settings with provided params
    let all_params = merge_params(provider, params)?;
    
    // Validate required parameters
    validate_params(action_def, &all_params)?;
    
    // Execute the action and its sub-actions
    let result = execute_sub_action(action_def, &all_params)?;
    
    let duration = start.elapsed();
    info!("Action '{}' completed in {:?}", action_name, duration);
    
    Ok(result)
}

// Merge default provider settings with supplied parameters
fn merge_params(provider: &Provider, params: HashMap<String, Value>) -> Result<HashMap<String, Value>, CpiError> {
    let mut all_params = HashMap::new();
    
    // Apply default settings if available
    if let Some(defaults) = &provider.default_settings {
        debug!("Applying {} default settings from provider", defaults.len());
        for (key, value) in defaults {
            all_params.insert(key.clone(), value.clone());
        }
    }
    
    // Apply the provided params, which override defaults
    debug!("Applying {} user-provided parameters", params.len());
    for (key, value) in params {
        all_params.insert(key, value);
    }
    
    trace!("Final parameter set contains {} parameters", all_params.len());
    Ok(all_params)
}

// Validate parameters against action requirements
fn validate_params(action_def: &ActionDef, params: &HashMap<String, Value>) -> Result<(), CpiError> {
    if let Some(required_params) = &action_def.params {
        debug!("Validating {} required parameters", required_params.len());
        
        for param in required_params {
            if !params.contains_key(param) {
                warn!("Missing required parameter: {}", param);
                return Err(CpiError::MissingParameter(param.clone()));
            }
        }
        
        trace!("All required parameters present");
    } else {
        trace!("No required parameters specified for this action");
    }
    
    Ok(())
}

// Execute a single action or sub-action
fn execute_sub_action(action_def: &ActionDef, params: &HashMap<String, Value>) -> Result<Value, CpiError> {
    let start = Instant::now();
    
    // Execute pre-exec actions if any
    if let Some(pre_actions) = &action_def.pre_exec {
        debug!("Executing {} pre-exec actions", pre_actions.len());
        for (index, sub_action) in pre_actions.iter().enumerate() {
            trace!("Executing pre-exec action #{}", index + 1);
            validate_params(sub_action, params)?;
            execute_sub_action(sub_action, params)?;
        }
    }
    
    // Execute the main command
    debug!("Executing main command");

    let result: Value;
    match &action_def.target {
        ActionTarget::Command(command) => {
            let cmd = fill_template(command, params)?;

            // Here it is, the call you have been digging around for
            // This is where we actually run the assembled command
            // defined by the CPI ini the system shell.
            let output = execute_command(&cmd)?;
    
            // Parse the output according to the parse rules
            debug!("Parsing command output ({} bytes)", output.len());
            result = match parser::parse_output(&output, &action_def.parse_rules, params) {
                Ok(value) => value,
                Err(e) => {
                    error!("Failed to parse command output: {}", e);
                    error!("Command output was: {}", truncate_output(&output, 1000));
                    return Err(e);
                }
            };
        },
        ActionTarget::Endpoint { url, method, headers } => {

            error!("No endpoint defined for action '{}'", action_def);
            return Err(CpiError::ExecutionFailed(format!("No endpoint defined for action '{}'", action_def)));
        }
    }
    
    

    
    // Execute post-exec actions if any
    if let Some(post_actions) = &action_def.post_exec {
        debug!("Executing {} post-exec actions", post_actions.len());
        for (index, sub_action) in post_actions.iter().enumerate() {
            trace!("Executing post-exec action #{}", index + 1);
            validate_params(sub_action, params)?;
            execute_sub_action(sub_action, params)?;
        }
    }
    
    let duration = start.elapsed();
    debug!("Sub-action execution completed in {:?}", duration);
    
    Ok(result)
}

// Helper function to show truncated output in logs
fn truncate_output(output: &str, max_len: usize) -> String {
    if output.len() <= max_len {
        output.to_string()
    } else {
        format!("{}... [truncated, {} bytes total]", &output[..max_len], output.len())
    }
}

// Helper function to fill in a command template with params
fn fill_template(template: &str, params: &HashMap<String, Value>) -> Result<String, CpiError> {
    trace!("Filling template: {}", template);
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
    
    trace!("Filled template: {}", result);
    Ok(result)
}

// Helper function to execute a command using run_script
fn execute_command(cmd: &str) -> Result<String, CpiError> {
    debug!("Executing shell command: {}", cmd);
    let start = Instant::now();
    
    // Configure script options
    let options = ScriptOptions::new();
    
    // No arguments needed
    let args: Vec<String> = vec![];
    
    // Run the script using run_script crate
    match run_script::run(cmd, &args, &options) {
        Ok((code, output, error)) => {
            let duration = start.elapsed();
            
            debug!("Command completed in {:?}", duration);
            debug!("Exit code: {}", code);
            
            if !error.is_empty() {
                trace!("Command stderr: {}", truncate_output(&error, 200));
            }
            
            if code == 0 {
                debug!("Command succeeded with output: {} bytes", output.len());
                trace!("Command output: {}", truncate_output(&output, 200));
                Ok(output)
            } else {
                error!("Command failed with exit code: {}", code);
                error!("Command error output: {}", truncate_output(&error, 500));
                
                Err(CpiError::ExecutionFailed(format!(
                    "Command failed with exit code {}: {}\nError: {}", 
                    code, cmd, error
                )))
            }
        },
        Err(e) => {
            error!("Failed to execute command: {}", e);
            Err(CpiError::ExecutionFailed(format!(
                "Failed to execute command '{}': {}", cmd, e
            )))
        }
    }
}
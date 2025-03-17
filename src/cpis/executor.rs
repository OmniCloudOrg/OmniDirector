// executor.rs - Optimized for performance and reliability
use super::provider::{Provider, ActionDef};
use super::error::CpiError;
use super::parser;
use log::{info, debug, trace, warn, error};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::time::Instant;
use std::io::Read;
use serde_json::Value;

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
    let cmd = fill_template(&action_def.command, params)?;
    let output = execute_command(&cmd)?;
    
    // Parse the output according to the parse rules
    debug!("Parsing command output ({} bytes)", output.len());
    let result = match parser::parse_output(&output, &action_def.parse_rules, params) {
        Ok(value) => value,
        Err(e) => {
            error!("Failed to parse command output: {}", e);
            error!("Command output was: {}", truncate_output(&output, 1000));
            return Err(e);
        }
    };
    
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

// Helper function to execute a command with better error handling
fn execute_command(cmd: &str) -> Result<String, CpiError> {
    debug!("Executing command: {}", cmd);
    let start = Instant::now();
    
    // Parse the command into executable and arguments
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    
    if parts.is_empty() {
        return Err(CpiError::ExecutionFailed("Empty command".to_string()));
    }
    
    let executable = parts[0];
    let args = &parts[1..];
    
    debug!("Executing: {} with {} arguments", executable, args.len());
    
    // Create and execute the command
    let mut command = Command::new(executable);
    command.args(args)
           .stdin(Stdio::null())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());
    
    // Use spawn+wait instead of output() for better error handling
    let mut child = command.spawn().map_err(|e| {
        error!("Failed to spawn command '{}': {}", executable, e);
        CpiError::ExecutionFailed(format!("Failed to execute '{}': {}", cmd, e))
    })?;
    
    // Get the exit status and capture output
    let status = child.wait().map_err(|e| {
        error!("Failed to wait for command completion: {}", e);
        CpiError::ExecutionFailed(format!("Failed to complete command '{}': {}", cmd, e))
    })?;
    
    // Read stdout and stderr
    let mut stdout = String::new();
    if let Some(mut stdout_handle) = child.stdout.take() {
        stdout_handle.read_to_string(&mut stdout).map_err(|e| {
            error!("Failed to read command stdout: {}", e);
            CpiError::ExecutionFailed(format!("Failed to read stdout: {}", e))
        })?;
    }
    
    let mut stderr = String::new();
    if let Some(mut stderr_handle) = child.stderr.take() {
        stderr_handle.read_to_string(&mut stderr).map_err(|e| {
            error!("Failed to read command stderr: {}", e);
            CpiError::ExecutionFailed(format!("Failed to read stderr: {}", e))
        })?;
    }
    
    let duration = start.elapsed();
    
    if !status.success() {
        error!("Command failed with status: {}", status);
        error!("Command stderr: {}", truncate_output(&stderr, 500));
        return Err(CpiError::ExecutionFailed(format!(
            "Command failed with status {}: {}\nError: {}", 
            status, cmd, stderr
        )));
    }
    
    debug!("Command succeeded in {:?} with output: {} bytes", duration, stdout.len());
    trace!("Command output: {}", truncate_output(&stdout, 200));
    
    Ok(stdout)
}
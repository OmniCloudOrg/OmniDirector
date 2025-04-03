// executor.rs - Using run_script crate for reliable shell execution
use super::error::CpiError;
use super::parser;
use super::provider::{ActionDef, ActionTarget, Provider};
use log::{debug, error, info, trace, warn};
use run_script::ScriptOptions;
use serde_json::Value;
use std::collections::HashMap;
use ssh2::Session;
use std::io::prelude::*;
use std::net::TcpStream;
use std::time::Duration;
use std::time::Instant;
use std::env;

// Main function to execute a CPI action
pub fn execute_action(
    provider: &Provider,
    action_name: &str,
    params: HashMap<String, Value>,
) -> Result<Value, CpiError> {
    let start = Instant::now();
    debug!(
        "Executing action '{}' from provider '{}'",
        action_name, provider.name
    );

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
fn merge_params(
    provider: &Provider,
    params: HashMap<String, Value>,
) -> Result<HashMap<String, Value>, CpiError> {
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

    trace!(
        "Final parameter set contains {} parameters",
        all_params.len()
    );
    Ok(all_params)
}

// Validate parameters against action requirements
fn validate_params(
    action_def: &ActionDef,
    params: &HashMap<String, Value>,
) -> Result<(), CpiError> {
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
fn execute_sub_action(
    action_def: &ActionDef,
    params: &HashMap<String, Value>,
) -> Result<Value, CpiError> {
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
            let cmd = fill_template(&command.command, params)?;

            // Here it is, the call you have been digging around for
            // This is where we actually run the assembled command
            // defined by the CPI ini the system shell.
            let output;
            if command.in_vm.unwrap_or(false) {
                // If the command is to be run in a VM, we need to use the VM's shell
                // This is a placeholder for actual VM execution logic
                // You would need to implement this based on your VM management library
                output = execute_command_vm(&cmd)?;
            } else {
                // If the command is to be run on the host, we can use the system shell
                output = execute_command(&cmd)?;
            }

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
        }
        ActionTarget::Endpoint {
            url,
            method,
            headers,
            body,
        } => {
            // Fill the URL and body templates with parameters
            let filled_url = fill_template(url, params)?;
            let filled_body = fill_template(body.as_deref().unwrap_or(""), params)?;
            let filled_headers = headers
                .as_ref()
                .map(|h| {
                    h.iter()
                        .map(|(key, value)| {
                            let filled_key = fill_template(key, params)?;
                            let filled_value = fill_template(value, params)?;
                            Ok((filled_key, filled_value))
                        })
                        .collect::<Result<HashMap<_, _>, CpiError>>()
                })
                .transpose()?;

            // Execute the HTTP request
            debug!(
                "Executing HTTP request to {} with method {:?}",
                filled_url, method
            );
            let response = reqwest::blocking::Client::new()
                .request(
                    method.to_string().parse().map_err(|_| {
                        CpiError::InvalidParameterType(
                            "method".to_string(),
                            "valid HTTP method".to_string(),
                        )
                    })?,
                    &filled_url,
                )
                .headers(
                    filled_headers.map_or_else(reqwest::header::HeaderMap::new, |headers| {
                        headers
                            .into_iter()
                            .filter_map(|(key, value)| {
                                let header_name =
                                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).ok()?;
                                let header_value =
                                    reqwest::header::HeaderValue::from_str(&value).ok()?;
                                Some((header_name, header_value))
                            })
                            .collect()
                    }),
                )
                .body(filled_body)
                .send()
                .map_err(|e| CpiError::ExecutionFailed(format!("HTTP request failed: {}", e)))?;

            // Check for HTTP errors
            if !response.status().is_success() {
                return Err(CpiError::ExecutionFailed(format!(
                    "HTTP request failed with status: {}",
                    response.status()
                )));
            }
            // Parse the response body
            let response_body = response.text().map_err(|e| {
                CpiError::ExecutionFailed(format!("Failed to read response body: {}", e))
            })?;
            debug!(
                "HTTP response body: {}",
                truncate_output(&response_body, 1000)
            );

            // Parse the response according to the parse rules
            result = match parser::parse_output(&response_body, &action_def.parse_rules, params) {
                Ok(value) => value,
                Err(e) => {
                    error!("Failed to parse HTTP response: {}", e);
                    error!(
                        "Response body was: {}",
                        truncate_output(&response_body, 1000)
                    );
                    return Err(e);
                }
            };
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
        format!(
            "{}... [truncated, {} bytes total]",
            &output[..max_len],
            output.len()
        )
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
        }
        Err(e) => {
            error!("Failed to execute command: {}", e);
            Err(CpiError::ExecutionFailed(format!(
                "Failed to execute command '{}': {}",
                cmd, e
            )))
        }
    }
}

/// Execute a command on a remote VM via SSH using the ssh2 crate
/// 
/// Retrieves SSH credentials from environment variables:
/// - OMNI_SSH_HOST: Hostname or IP address
/// - OMNI_SSH_USER: SSH username
/// - OMNI_SSH_PASSWORD: SSH password
/// 
/// # Arguments
/// * `command` - The command to execute on the remote VM
/// 
/// # Returns
/// * The command output as a string if successful
/// * A CpiError if any part of the operation fails
pub fn execute_command_vm(command: &str) -> Result<String, CpiError> {
    println!("Executing command on VM: {}", command);

    // Get SSH credentials from environment variables
    let host = env::var("OMNI_SSH_HOST").map_err(|_| {
        let err_msg = "Missing OMNI_SSH_HOST environment variable";
        error!("{}", err_msg);
        CpiError::ExecutionFailed(err_msg.to_string())
    })?;
    
    let username = env::var("OMNI_SSH_USER").map_err(|_| {
        let err_msg = "Missing OMNI_SSH_USER environment variable";
        error!("{}", err_msg);
        CpiError::ExecutionFailed(err_msg.to_string())
    })?;
    
    let password = env::var("OMNI_SSH_PASSWORD").map_err(|_| {
        let err_msg = "Missing OMNI_SSH_PASSWORD environment variable";
        error!("{}", err_msg);
        CpiError::ExecutionFailed(err_msg.to_string())
    })?;
    
    debug!("Executing VM command on host '{}' as user '{}'", host, username);
    let start = Instant::now();

    // Connect with timeout
    let tcp = match TcpStream::connect_timeout(
        &format!("{}:22", host).parse().map_err(|e| {
            error!("Invalid host address: {}", e);
            CpiError::ExecutionFailed(format!("Invalid host address: {}", e))
        })?,
        Duration::from_secs(10)
    ) {
        Ok(stream) => {
            // Set read/write timeouts
            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
            stream
        },
        Err(e) => {
            error!("Failed to connect to SSH server: {}", e);
            return Err(CpiError::ExecutionFailed(
                format!("Failed to connect to SSH server {}: {}", host, e)
            ));
        }
    };

    // Create SSH session
    let mut sess = Session::new().map_err(|e| {
        error!("Failed to create SSH session: {}", e);
        CpiError::ExecutionFailed(format!("Failed to create SSH session: {}", e))
    })?;

    sess.set_tcp_stream(tcp);
    sess.handshake().map_err(|e| {
        error!("SSH handshake failed: {}", e);
        CpiError::ExecutionFailed(format!("SSH handshake failed: {}", e))
    })?;

    // Authenticate with password
    sess.userauth_password(&username, &password).map_err(|e| {
        error!("SSH authentication failed: {}", e);
        CpiError::ExecutionFailed(format!("SSH authentication failed: {}", e))
    })?;

    // Execute command
    debug!("Opening SSH channel");
    let mut channel = sess.channel_session().map_err(|e| {
        error!("Failed to open SSH channel: {}", e);
        CpiError::ExecutionFailed(format!("Failed to open SSH channel: {}", e))
    })?;

    debug!("Executing command: {}", command);
    channel.exec(command).map_err(|e| {
        error!("Failed to execute command: {}", e);
        CpiError::ExecutionFailed(format!("Failed to execute command: {}", e))
    })?;

    // Read output
    let mut output = String::new();
    channel.read_to_string(&mut output).map_err(|e| {
        error!("Failed to read command output: {}", e);
        CpiError::ExecutionFailed(format!("Failed to read command output: {}", e))
    })?;

    // Read stderr
    let mut stderr = String::new();
    channel.stderr().read_to_string(&mut stderr).map_err(|e| {
        error!("Failed to read stderr: {}", e);
        CpiError::ExecutionFailed(format!("Failed to read stderr: {}", e))
    })?;

    // Get exit status
    channel.wait_close().map_err(|e| {
        error!("Failed to close SSH channel: {}", e);
        CpiError::ExecutionFailed(format!("Failed to close SSH channel: {}", e))
    })?;

    let exit_status = channel.exit_status().map_err(|e| {
        error!("Failed to get exit status: {}", e);
        CpiError::ExecutionFailed(format!("Failed to get exit status: {}", e))
    })?;

    let duration = start.elapsed();
    debug!("Command completed in {:?} with status {}", duration, exit_status);

    if !stderr.is_empty() {
        debug!("Command stderr: {}", truncate_output(&stderr, 200));
    }

    if exit_status == 0 {
        debug!("Command succeeded with output: {} bytes", output.len());
        trace!("Command output: {}", truncate_output(&output, 200));
        Ok(output)
    } else {
        error!("Command failed with exit code: {}", exit_status);
        error!("Command stderr: {}", truncate_output(&stderr, 500));
        Err(CpiError::ExecutionFailed(format!(
            "Command failed with exit code {}: {}",
            exit_status, stderr
        )))
    }
}
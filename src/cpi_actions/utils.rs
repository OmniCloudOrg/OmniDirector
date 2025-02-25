use serde_json::{Map, Value};
use std::process::{Command, Output};
use anyhow::{Context, Result};

/// Execute a shell command with arguments
pub fn execute_shell_cmd(command_str: &mut String) -> Result<Output> {
    let mut parts = command_str.splitn(2, ' ');
    let executable = parts.next().unwrap_or("");
    let args = parts.next().unwrap_or("");

    let output = Command::new(executable)
        .args(args.split_whitespace())
        .output()
        .context("failed to execute command")?;

    Ok(output)
}

/// Replace template parameters in a command string
pub fn replace_template_params(params: &Map<String, Value>, command_str: &mut String) -> String {
    for (key, value) in params {
        let placeholder = format!("{{{}}}", key);
        let replacement = match value {
            Value::String(s) => s.to_owned(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Object(obj) => serde_json::to_string(&obj)
                .unwrap_or_default()
                .trim_matches(|c| c == '{' || c == '}')
                .to_string(),
            Value::Array(arr) => serde_json::to_string(&arr)
                .unwrap_or_default()
                .trim_matches(|c| c == '[' || c == ']')
                .to_string(),
            Value::Null => "null".to_string(),
        };
        *command_str = command_str.replace(&placeholder, &replacement);
    }
    command_str.to_string()
}
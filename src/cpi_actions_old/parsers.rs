use serde::Deserialize;
use std::collections::HashMap;
use std::process::Output;
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use chrono::{DateTime, Utc};
use regex::Regex;

/// Configuration for parsing command output
#[derive(Debug, Deserialize)]
pub struct OutputParser {
    #[serde(rename = "type")]
    pub parser_type: String,
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub capture_groups: HashMap<String, i32>,
    #[serde(default)]
    pub patterns: HashMap<String, PatternConfig>,
    #[serde(default)]
    pub delimiter: String,
    #[serde(default)]
    pub fields: HashMap<String, FieldConfig>,
    #[serde(default)]
    pub success_value: i32,
}

/// Configuration for a regex pattern with transformation options
#[derive(Debug, Deserialize)]
pub struct PatternConfig {
    #[serde(default)]
    pub transform: Option<String>,
    pub capture_group: i32,
    pub pattern: String,
}

/// Configuration for a field to be extracted from command output
#[derive(Debug, Deserialize)]
pub struct FieldConfig {
    #[serde(default)]
    pub transform: Option<String>,
    pub capture_group: i32,
    pub pattern: String,
}

/// Command to be executed after the main command
#[derive(Debug, Deserialize)]
pub enum PostExecCommand {
    /// Custom command with an explicit output parser for advanced use.
    Custom {
        output_parser: OutputParser,
        command: String,
    },
    /// Command utilizing defaults
    Basic(String),
}


/// Parse command output according to the specified parser configuration
pub fn parse_command_output(output: &Output, parser: &OutputParser) -> Result<Value> {
    match parser.parser_type.as_str() {
        "exit_code" => {
            if output.status.code().unwrap_or(-1) == parser.success_value {
                Ok(Value::Bool(true))
            } else {
                let error_msg = String::from_utf8(output.stderr.clone())
                    .context("failed to parse stderr as UTF-8")?;
                Err(anyhow::anyhow!(error_msg))
            }
        }
        "regex" => {
            let output_str = String::from_utf8(output.stdout.clone())
                .context("failed to parse stdout as UTF-8")?;
            let re = Regex::new(&parser.pattern)
                .context("failed to compile regex pattern")?;
            
            let captures = re.captures(&output_str)
                .context("regex pattern did not match output")?;
            
            let mut result = Map::new();
            for (key, group_num) in &parser.capture_groups {
                if let Some(capture) = captures.get(*group_num as usize) {
                    result.insert(key.clone(), Value::String(capture.as_str().to_string()));
                }
            }
            Ok(Value::Object(result))
        }
        "multi_regex" => {
            let output_str = String::from_utf8(output.stdout.clone())
                .context("failed to parse stdout as UTF-8")?;
            let mut result = Map::new();
            
            for (key, pattern_config) in &parser.patterns {
                let re = Regex::new(&pattern_config.pattern)
                    .context(format!("failed to compile regex pattern for {}", key))?;
                
                if let Some(captures) = re.captures(&output_str) {
                    if let Some(capture) = captures.get(pattern_config.capture_group as usize) {
                        let value = if let Some(transform) = &pattern_config.transform {
                            transform_value(capture.as_str(), transform)?
                        } else {
                            Value::String(capture.as_str().to_string())
                        };
                        result.insert(key.clone(), value);
                    }
                }
            }
            Ok(Value::Object(result))
        }
        "table" => {
            let output_str = String::from_utf8(output.stdout.clone())
                .context("failed to parse stdout as UTF-8")?;
            let sections: Vec<&str> = output_str.split(&parser.delimiter).collect();
            let mut results = Vec::new();
            
            for section in sections {
                if section.trim().is_empty() {
                    continue;
                }
                
                let mut row = Map::new();
                for (field_name, field_config) in &parser.fields {
                    let re = Regex::new(&field_config.pattern)
                        .context(format!("failed to compile regex pattern for {}", field_name))?;
                    
                    if let Some(captures) = re.captures(section) {
                        if let Some(capture) = captures.get(field_config.capture_group as usize) {
                            let value = if let Some(transform) = &field_config.transform {
                                transform_value(capture.as_str(), transform)?
                            } else {
                                Value::String(capture.as_str().to_string())
                            };
                            row.insert(field_name.clone(), value);
                        }
                    }
                }
                if !row.is_empty() {
                    results.push(Value::Object(row));
                }
            }
            Ok(Value::Array(results))
        }
        _ => Err(anyhow::anyhow!("Unknown parser type: {}", parser.parser_type)),
    }
}

/// Transform a string value according to the specified transformation
pub fn transform_value(value: &str, transform: &str) -> Result<Value> {
    match transform {
        "int"      => Ok(Value::Number(value.parse().context("failed to parse int")?)),
        "float"    => Ok(Value::Number(value.parse().context("failed to parse float")?)),
        "boolean"  => Ok(Value::Bool(value.to_lowercase() == "true")),
        "datetime" => {
            let dt = DateTime::parse_from_rfc3339(value)
                .or_else(|_| DateTime::parse_from_rfc2822(value))
                .context("failed to parse datetime")?;
            Ok(Value::String(dt.with_timezone(&Utc).to_rfc3339()))
        }
        "array" => Ok(Value::Array(
            value.split(',')
                .map(|s| Value::String(s.trim().to_string()))
                .collect()
        )),
        _ => Err(anyhow::anyhow!("Unknown transform type: {}", transform)),
    }
}
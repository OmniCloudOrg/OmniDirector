use anyhow::{Error, Result};
use chrono::Local;
use colored::Colorize;
use serde_json::Value;
use std::fmt::Display;

pub struct Logger {
    show_timestamp: bool,
}

impl Logger {
    pub fn new(show_timestamp: bool) -> Self {
        Self { show_timestamp }
    }

    fn timestamp(&self) -> String {
        if self.show_timestamp {
            format!("[{}] ", Local::now().format("%Y-%m-%d %H:%M:%S%.3f"))
                .bright_black()
                .to_string()
        } else {
            String::new()
        }
    }

    pub fn info<T: Display>(&self, message: T) {
        println!(
            "{}{}{}",
            self.timestamp(),
            "INFO ".bright_blue().bold(),
            message
        );
    }

    pub fn success<T: Display>(&self, message: T) {
        println!(
            "{}{}{}",
            self.timestamp(),
            "SUCCESS ".bright_green().bold(),
            message
        );
    }

    pub fn warn<T: Display>(&self, message: T) {
        println!("{}{}{}", self.timestamp(), "WARN ".yellow().bold(), message);
    }

    pub fn error<T: Display>(&self, message: T) {
        eprintln!(
            "{}{}{}",
            self.timestamp(),
            "ERROR ".bright_red().bold(),
            message
        );
    }

    pub fn debug<T: Display>(&self, message: T) {
        println!(
            "{}{}{}",
            self.timestamp(),
            "DEBUG ".bright_magenta().bold(),
            message
        );
    }

    // Pretty print JSON with proper indentation and colors
    pub fn json(&self, prefix: &str, value: &Value) {
        let formatted = self.format_json(value, 0);
        println!(
            "{}{}{}\n{}",
            self.timestamp(),
            "JSON ".bright_cyan().bold(),
            prefix,
            formatted
        );
    }

    // Format JSON with colored output and proper indentation
    fn format_json(&self, value: &Value, indent_level: usize) -> String {
        let indent = "  ".repeat(indent_level);
        let next_indent = "  ".repeat(indent_level + 1);

        match value {
            Value::Object(map) => {
                let mut result = String::from("{\n");
                let mut first = true;
                for (key, value) in map {
                    if !first {
                        result.push_str(",\n");
                    }
                    first = false;
                    result.push_str(&format!(
                        "{}{}: {}",
                        next_indent,
                        key.bright_yellow(),
                        self.format_json(value, indent_level + 1)
                    ));
                }
                result.push_str(&format!("\n{}}}", indent));
                result
            }
            Value::Array(arr) => {
                let mut result = String::from("[\n");
                let mut first = true;
                for value in arr {
                    if !first {
                        result.push_str(",\n");
                    }
                    first = false;
                    result.push_str(&format!(
                        "{}{}",
                        next_indent,
                        self.format_json(value, indent_level + 1)
                    ));
                }
                result.push_str(&format!("\n{}]", indent));
                result
            }
            Value::String(s) => format!("{}", s.bright_green()),
            Value::Number(n) => format!("{}", n.to_string().bright_cyan()),
            Value::Bool(b) => format!("{}", b.to_string().bright_blue()),
            Value::Null => format!("{}", "null".bright_red()),
        }
    }

    // Pretty print errors with cause chain
    pub fn error_chain(&self, error: &Error) {
        let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error.as_ref());
        let mut index = 0;

        eprintln!(
            "{}{} Error chain:",
            self.timestamp(),
            "ERROR".bright_red().bold()
        );

        while let Some(err) = current {
            eprintln!(
                "{}  {}{}",
                self.timestamp(),
                "→".bright_red(),
                format!("[{}] {}", index, err).bright_red()
            );

            if let Some(cause) = err.source() {
                eprintln!(
                    "{}    {}{}",
                    self.timestamp(),
                    "caused by: ".bright_black(),
                    cause
                );
            }

            current = err.source();
            index += 1;
        }
    }

    // Helper method to pretty print any JSON-serializable value
    pub fn pretty_json<T: serde::Serialize>(&self, prefix: &str, value: &T) -> Result<()> {
        let json_value = serde_json::to_value(value)?;
        self.json(prefix, &json_value);
        Ok(())
    }
}

// Example usage:
pub fn example() -> Result<()> {
    let logger = Logger::new(true);

    // Basic logging
    logger.info("Starting application...");
    logger.success("Successfully connected to database");
    logger.warn("Cache miss detected");
    logger.error("Failed to connect to external service");
    logger.debug("Current memory usage: 156MB");

    // JSON logging
    let json_value: Value = serde_json::json!({
        "name": "test-vm",
        "config": {
            "memory_mb": 4096,
            "cpu_count": 8,
            "networks": ["VM Network"],
            "disks": [
                {
                    "size_mb": 10240,
                    "path": "/vmfs/volumes/datastore1/test-vm/test-disk.vmdk"
                }
            ]
        }
    });
    logger.json("VM Configuration:", &json_value);

    // Error chain logging
    let error = anyhow::anyhow!("Failed to create VM")
        .context("Infrastructure error")
        .context("Deployment failed");
    logger.error_chain(&error);

    Ok(())
}

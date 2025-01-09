use anyhow::{Context, Result};
use colored::Colorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::process::{Command, Output};
use chrono::{DateTime, Utc};
use ez_logging::println;

#[derive(Debug, Deserialize)]
struct OutputParser {
    #[serde(rename = "type")]
    parser_type: String,
    #[serde(default)]
    pattern: String,
    #[serde(default)]
    capture_groups: HashMap<String, i32>,
    #[serde(default)]
    patterns: HashMap<String, PatternConfig>,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    fields: HashMap<String, FieldConfig>,
    #[serde(default)]
    success_value: i32,
}

#[derive(Debug, Deserialize)]
struct PatternConfig {
    pattern: String,
    capture_group: i32,
    #[serde(default)]
    transform: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FieldConfig {
    pattern: String,
    capture_group: i32,
    #[serde(default)]
    transform: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostExecCommand {
    command: String,
    #[serde(default)]
    output_parser: Option<OutputParser>,
}

pub struct CpiCommand {
    pub config: String,
}

impl CpiCommand {
    pub fn new() -> Result<Self> {
        let config_str = fs::read_to_string("./CPIs/cpi-virtualbox.json")?;
        let config_json: Value = serde_json::from_str(&config_str)?;

        Ok(Self {
            config: config_json.to_string(),
        })
    }

    pub fn execute(&self, command: CpiCommandType) -> Result<Value> {
        // Parse config
        let config_json: Value =
            serde_json::from_str(&self.config).context("failed to deserialize json")?;
        
        let actions = config_json
            .get("actions")
            .context("'actions' was not defined in the config")?;

        let command_type = actions.get(command.to_string()).context(format!(
            "Command type not found for '{}'",
            command.to_string()
        ))?;

        // Get command template and parser config
        let command_template = command_type
            .get("command")
            .context("'command' field not found for command type")?
            .as_str()
            .unwrap();

        let output_parser = command_type
            .get("output_parser")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        // Get the post-exec commands if they exist
        let post_exec_commands = match command_type.get("post_exec") {
            Some(post_exec) => {
                post_exec
                    .as_array()
                    .context("Post exec commands found but were not an array")?
                    .iter()
                    .map(|v| serde_json::from_value(v.clone()))
                    .collect::<Result<Vec<PostExecCommand>, _>>()?
            }
            None => Vec::new(),
        };

        // Get pre-attach commands if they exist
        let pre_attach_commands = match command_type.get("pre_attach") {
            Some(pre_attach) => {
                pre_attach
                    .as_array()
                    .context("Pre attach commands found but were not an array")?
                    .iter()
                    .map(|v| serde_json::from_value(v.clone()))
                    .collect::<Result<Vec<PostExecCommand>, _>>()?
            }
            None => Vec::new(),
        };

        // Extract and validate parameters
        let params: Value = serde_json::to_value(&command).context("failed to serialize command")?;
        println!("Command type: {}", command.to_string().green());
        println!("Command parameters: {}", &params);

        let params = if params.as_object().map_or(true, |obj| obj.is_empty()) {
            &Map::new()
        } else {
            params
                .as_object()
                .and_then(|obj| obj.values().next())
                .and_then(|v| v.as_object())
                .context("failed to extract params from command")?
        };

        // Execute pre-attach commands if they exist
        for pre_attach_cmd in pre_attach_commands {
            let mut cmd_str = replace_template_params(params, &mut pre_attach_cmd.command.clone());
            let output = execute_shell_cmd(&mut cmd_str)?;
            
            if let Some(parser) = pre_attach_cmd.output_parser {
                parse_command_output(&output, &parser)?;
            } else if !output.status.success() {
                let error_msg = String::from_utf8(output.stderr)
                    .context("failed to parse pre-attach stderr as UTF-8")?;
                return Err(anyhow::anyhow!(error_msg));
            }
        }

        // Execute main command
        let mut command_str = replace_template_params(params, &mut command_template.to_string());
        let output = execute_shell_cmd(&mut command_str)?;

        // Parse main command output
        let result = if let Some(parser) = output_parser {
            parse_command_output(&output, &parser)?
        } else {
            // Default behavior if no parser specified
            if !output.status.success() {
                let error_msg = String::from_utf8(output.stderr)
                    .context("failed to parse stderr as UTF-8")?;
                return Err(anyhow::anyhow!(error_msg));
            }
            let output_str = String::from_utf8(output.stdout)
                .context("failed to parse stdout as UTF-8")?;
            serde_json::from_str(&format!(r#"{{"result": {}}}"#, 
                serde_json::to_string(&output_str)?))?
        };

        // Execute post-exec commands
        for post_exec_cmd in post_exec_commands {
            let mut cmd_str = replace_template_params(params, &mut post_exec_cmd.command.clone());
            let output = execute_shell_cmd(&mut cmd_str)?;
            
            if let Some(parser) = post_exec_cmd.output_parser {
                parse_command_output(&output, &parser)?;
            } else if !output.status.success() {
                let error_msg = String::from_utf8(output.stderr)
                    .context("failed to parse post-exec stderr as UTF-8")?;
                return Err(anyhow::anyhow!(error_msg));
            }
        }

        Ok(result)
    }
}

fn execute_shell_cmd(command_str: &mut String) -> Result<Output> {
    let mut parts = command_str.splitn(2, ' ');
    let executable = parts.next().unwrap_or("");
    let args = parts.next().unwrap_or("");

    let output = Command::new(executable)
        .args(args.split_whitespace())
        .output()
        .context("failed to execute command")?;

    Ok(output)
}

fn parse_command_output(output: &Output, parser: &OutputParser) -> Result<Value> {
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

fn transform_value(value: &str, transform: &str) -> Result<Value> {
    match transform {
        "int" => Ok(Value::Number(value.parse().context("failed to parse int")?)),
        "float" => Ok(Value::Number(value.parse().context("failed to parse float")?)),
        "boolean" => Ok(Value::Bool(value.to_lowercase() == "true")),
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

fn replace_template_params(params: &Map<String, Value>, command_str: &mut String) -> String {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CpiCommandType {
    #[serde(rename = "test_install")]
    TestInstall,
    #[serde(rename = "create_vm")]
    CreateVm {
        vm_name: String,
        os_type: String,
        memory_mb: u32,
        cpu_count: u32,
    },
    #[serde(rename = "delete_vm")]
    DeleteVm {
        vm_name: String,
    },
    #[serde(rename = "has_vm")]
    HasVm {
        vm_id: String,
    },
    #[serde(rename = "start_vm")]
    StartVm {
        vm_name: String,
    },
    #[serde(rename = "configure_networks")]
    ConfigureNetworks {
        vm_name: String,
        network_index: u32,
        network_type: String,
    },
    #[serde(rename = "create_disk")]
    CreateDisk {
        disk_path: String,
        size_mb: u64,
    },
    #[serde(rename = "delete_disk")]
    DeleteDisk {
        disk_path: String,
    },
    #[serde(rename = "attach_disk")]
    AttachDisk {
        vm_name: String,
        controller_name: String,
        port: u32,
        disk_path: String,
    },
    #[serde(rename = "detach_disk")]
    DetachDisk {
        vm_name: String,
        controller_name: String,
        port: u32,
    },
    #[serde(rename = "has_disk")]
    HasDisk {
        disk_path: String,
    },
    #[serde(rename = "set_vm_metadata")]
    SetVmMetadata {
        vm_name: String,
        key: String,
        value: String,
    },
    #[serde(rename = "create_snapshot")]
    CreateSnapshot {
        vm_name: String,
        snapshot_name: String,
    },
    #[serde(rename = "delete_snapshot")]
    DeleteSnapshot {
        vm_name: String,
        snapshot_name: String,
    },
    #[serde(rename = "has_snapshot")]
    HasSnapshot {
        vm_name: String,
        snapshot_name: String,
    },
    #[serde(rename = "get_disks")]
    GetDisks,
    #[serde(rename = "get_vm")]
    GetVm {
        vm_name: String,
    },
    #[serde(rename = "reboot_vm")]
    RebootVm {
        vm_name: String,
    },
    #[serde(rename = "snapshot_disk")]
    SnapshotDisk {
        source_disk_path: String,
        target_disk_path: String,
    },
    #[serde(rename = "get_snapshots")]
    GetSnapshots {
        vm_name: String,
    },
}

impl ToString for CpiCommandType {
    fn to_string(&self) -> String {
        match self {
            CpiCommandType::TestInstall => "test_install".to_string(),
            CpiCommandType::CreateVm { .. } => "create_vm".to_string(),
            CpiCommandType::DeleteVm { .. } => "delete_vm".to_string(),
            CpiCommandType::HasVm { .. } => "has_vm".to_string(),
            CpiCommandType::StartVm { .. } => "start_vm".to_string(),
            CpiCommandType::ConfigureNetworks { .. } => "configure_networks".to_string(),
            CpiCommandType::CreateDisk { .. } => "create_disk".to_string(),
            CpiCommandType::DeleteDisk { .. } => "delete_disk".to_string(),
            CpiCommandType::AttachDisk { .. } => "attach_disk".to_string(),
            CpiCommandType::DetachDisk { .. } => "detach_disk".to_string(),
            CpiCommandType::HasDisk { .. } => "has_disk".to_string(),
            CpiCommandType::SetVmMetadata { .. } => "set_vm_metadata".to_string(),
            CpiCommandType::CreateSnapshot { .. } => "create_snapshot".to_string(),
            CpiCommandType::DeleteSnapshot { .. } => "delete_snapshot".to_string(),
            CpiCommandType::HasSnapshot { .. } => "has_snapshot".to_string(),
            CpiCommandType::GetDisks => "get_disks".to_string(),
            CpiCommandType::GetVm { .. } => "get_vm".to_string(),
            CpiCommandType::RebootVm { .. } => "reboot_vm".to_string(),
            CpiCommandType::SnapshotDisk { .. } => "snapshot_disk".to_string(),
            CpiCommandType::GetSnapshots { .. } => "get_snapshots".to_string(),
        }
    }
}

// Return types for VirtualBox-specific operations
#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualMachine {
    pub name: String,
    pub uuid: String,
    pub state: String,
    pub os_type: String,
    pub memory_mb: u32,
    pub cpu_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualDisk {
    pub uuid: String,
    pub location: String,
    pub state: String,
    pub size_mb: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub name: String,
    pub uuid: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

pub struct CpiApi {
    cmd: CpiCommand,
}

impl CpiApi {
    pub fn new() -> Result<Self> {
        Ok(Self {
            cmd: CpiCommand::new()?,
        })
    }

    pub fn test_install(&self) -> Result<String> {
        let result = self.cmd.execute(CpiCommandType::TestInstall)?;
        Ok(result.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string())
    }

    pub fn create_vm(
        &self,
        name: &str,
        os_type: &str,
        memory_mb: u32,
        cpu_count: u32,
    ) -> Result<VirtualMachine> {
        let result = self.cmd.execute(CpiCommandType::CreateVm {
            vm_name: name.to_string(),
            os_type: os_type.to_string(),
            memory_mb,
            cpu_count,
        })?;

        let vm_info = self.get_vm(name)?;
        Ok(vm_info)
    }

    pub fn delete_vm(&self, name: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::DeleteVm {
            vm_name: name.to_string(),
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    pub fn start_vm(&self, name: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::StartVm {
            vm_name: name.to_string(),
        })?;

        Ok(result.get("started_vm")
            .and_then(|v| v.as_str())
            .map(|s| s == name)
            .unwrap_or(false))
    }

    pub fn get_vm(&self, name: &str) -> Result<VirtualMachine> {
        let result = self.cmd.execute(CpiCommandType::GetVm {
            vm_name: name.to_string(),
        })?;

        let vm: VirtualMachine = serde_json::from_value(result)
            .context("failed to parse VM info")?;
        Ok(vm)
    }

    pub fn create_disk(&self, path: &str, size_mb: u64) -> Result<VirtualDisk> {
        let result = self.cmd.execute(CpiCommandType::CreateDisk {
            disk_path: path.to_string(),
            size_mb,
        })?;

        let disk: VirtualDisk = serde_json::from_value(result)
            .context("failed to parse disk info")?;
        Ok(disk)
    }

    pub fn get_disks(&self) -> Result<Vec<VirtualDisk>> {
        let result = self.cmd.execute(CpiCommandType::GetDisks)?;
        
        let disks: Vec<VirtualDisk> = serde_json::from_value(result)
            .context("failed to parse disk list")?;
        Ok(disks)
    }

    pub fn create_snapshot(&self, vm_name: &str, snapshot_name: &str) -> Result<Snapshot> {
        let result = self.cmd.execute(CpiCommandType::CreateSnapshot {
            vm_name: vm_name.to_string(),
            snapshot_name: snapshot_name.to_string(),
        })?;

        let snapshot: Snapshot = serde_json::from_value(result)
            .context("failed to parse snapshot info")?;
        Ok(snapshot)
    }

    pub fn get_snapshots(&self, vm_name: &str) -> Result<Vec<Snapshot>> {
        let result = self.cmd.execute(CpiCommandType::GetSnapshots {
            vm_name: vm_name.to_string(),
        })?;

        let snapshots: Vec<Snapshot> = serde_json::from_value(result)
            .context("failed to parse snapshot list")?;
        Ok(snapshots)
    }
}

// Example test function
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_manage_vm() -> Result<()> {
        let api = CpiApi::new()?;
        
        // Test VM creation
        let vm = api.create_vm("test-vm", "Ubuntu_64", 2048, 2)?;
        assert_eq!(vm.name, "test-vm");
        
        // Create and attach disk
        let disk = api.create_disk("/tmp/test-disk.vdi", 10240)?;
        
        // Start VM
        assert!(api.start_vm("test-vm")?);
        
        // Create snapshot
        let snapshot = api.create_snapshot("test-vm", "test-snapshot")?;
        assert_eq!(snapshot.name, "test-snapshot");
        
        // Cleanup
        api.delete_vm("test-vm")?;
        
        Ok(())
    }
}
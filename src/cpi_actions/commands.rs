use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use anyhow::{Context, Result};
use ez_logging::println;
use colored::Colorize;

use super::parsers::{OutputParser, PostExecCommand, parse_command_output};
use super::utils::{execute_shell_cmd, replace_template_params};

/// Main struct for handling CPI commands
pub struct CpiCommand {
    pub config: String,
}

impl CpiCommand {
    /// Create a new CpiCommand instance by loading the configuration file
    pub fn new() -> Result<Self> {
        let config_str = fs::read_to_string("./CPIs/cpi-virtualbox.json")?;
        let config_json: Value = serde_json::from_str(&config_str)?;

        Ok(Self {
            config: config_json.to_string(),
        })
    }

    /// Execute a CPI command based on its type
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

/// Enum representing all possible CPI command types with their parameters
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CpiCommandType {
    #[serde(rename = "test_install")]
    TestInstall,
    
    #[serde(rename = "create_vm")]
    CreateVm {
        vm_name:   String,
        os_type:   String,
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
        vm_name:       String,
        network_index: u32,
        network_type:  String,
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
        vm_name:         String,
        port:            u32,
        disk_path:       String,
        controller_name: String,
    },
    
    #[serde(rename = "detach_disk")]
    DetachDisk {
        vm_name:         String,
        controller_name: String,
        port:            u32,
    },
    
    #[serde(rename = "has_disk")]
    HasDisk {
        disk_path: String,
    },
    
    #[serde(rename = "set_vm_metadata")]
    SetVmMetadata {
        vm_name: String,
        key:     String,
        value:   String,
    },
    
    #[serde(rename = "create_snapshot")]
    CreateSnapshot {
        vm_name:       String,
        snapshot_name: String,
    },
    
    #[serde(rename = "delete_snapshot")]
    DeleteSnapshot {
        vm_name:       String,
        snapshot_name: String,
    },
    
    #[serde(rename = "has_snapshot")]
    HasSnapshot {
        vm_name:       String,
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
            CpiCommandType::TestInstall       { .. } => "test_install".to_string(),
            CpiCommandType::CreateVm          { .. } => "create_vm".to_string(),
            CpiCommandType::DeleteVm          { .. } => "delete_vm".to_string(),
            CpiCommandType::HasVm             { .. } => "has_vm".to_string(),
            CpiCommandType::StartVm           { .. } => "start_vm".to_string(),
            CpiCommandType::ConfigureNetworks { .. } => "configure_networks".to_string(),
            CpiCommandType::CreateDisk        { .. } => "create_disk".to_string(),
            CpiCommandType::DeleteDisk        { .. } => "delete_disk".to_string(),
            CpiCommandType::AttachDisk        { .. } => "attach_disk".to_string(),
            CpiCommandType::DetachDisk        { .. } => "detach_disk".to_string(),
            CpiCommandType::HasDisk           { .. } => "has_disk".to_string(),
            CpiCommandType::SetVmMetadata     { .. } => "set_vm_metadata".to_string(),
            CpiCommandType::CreateSnapshot    { .. } => "create_snapshot".to_string(),
            CpiCommandType::DeleteSnapshot    { .. } => "delete_snapshot".to_string(),
            CpiCommandType::HasSnapshot       { .. } => "has_snapshot".to_string(),
            CpiCommandType::GetDisks          { .. } => "get_disks".to_string(),
            CpiCommandType::GetVm             { .. } => "get_vm".to_string(),
            CpiCommandType::RebootVm          { .. } => "reboot_vm".to_string(),
            CpiCommandType::SnapshotDisk      { .. } => "snapshot_disk".to_string(),
            CpiCommandType::GetSnapshots      { .. } => "get_snapshots".to_string(),
        }
    }
}
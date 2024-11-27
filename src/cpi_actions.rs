use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::process::{Command, Output};
use crate::logging::Logger;
use ez_logging::println;
use debug_print::{
    debug_print as dprint,
    debug_println as dprintln,
    debug_eprint as deprint,
    debug_eprintln as deprintln,
};

pub struct CpiCommand {
    pub config: String,
}

impl CpiCommand {
    pub fn new() -> Result<Self> {
        let config_str = fs::read_to_string("./CPIs/cpi-vb-wsl.json")?;
        let config_json: Value = serde_json::from_str(&config_str)?;

        Ok(Self {
            config: config_json.to_string(),
        })
    }

    // Execute a CPI command by fetching the template from the CPI,
    // filling the params, and returning the output
    pub fn execute(&self, command: CpiCommandType) -> Result<Value> {
        let logger = Logger::new(true);
        
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
    
        // Get command template
        let command_template = command_type
            .get("command")
            .context("'command field not found for command type'")?
            .as_str()
            .unwrap();
    
        // Get the post-exec command templates if they exist
        let post_exec_templates = match command_type.get("post_exec") {
            Some(post_exec) => {
                post_exec
                    .as_array()
                    .context("Post exec commands found but were not an array")?
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .context("post exec command was not a valid string")
                            .map(|s| s.to_string())
                    })
                    .collect::<Result<Vec<String>>>()?
            }
            None => Vec::new(),
        };
    
        // Serialize the enum variant to a JSON Value and extract params
        let params: Value = serde_json::to_value(&command).context("failed to serialize command")?;
        logger.debug("Command parameters:");
        logger.json("Params", &params);
    
        let params = params
            .as_object()
            .and_then(|obj| obj.values().next())
            .and_then(|v| v.as_object())
            .context("failed to extract params from command")?;
    
        // Execute main command
        let mut command_str = replace_template_params(params, &mut command_template.to_string());
        logger.info(format!("Executing command: {}", command_str.green().bold()));
    
        let output = execute_shell_cmd(&mut command_str)?;
        
        // Check main command execution
        if !output.status.success() {
            let error_msg = String::from_utf8(output.stderr)
                .context("failed to parse stderr as UTF-8")?;
            logger.error(format!("Command failed: {}", error_msg));
            return Err(anyhow::anyhow!(error_msg));
        }
    
        // Parse the output of the main command
        let output_str = String::from_utf8(output.stdout)
            .context("failed to parse stdout as UTF-8")?;
    
        // Execute post-exec commands if they exist
        if !post_exec_templates.is_empty() {
            logger.info("Executing post-exec commands...");
            for (index, post_exec_template) in post_exec_templates.iter().enumerate() {
                let mut post_exec_command = replace_template_params(params, &mut post_exec_template.to_string());
                logger.debug(format!("Post-exec command {}/{}: {}", 
                    index + 1, 
                    post_exec_templates.len(),
                    post_exec_command.green().bold()
                ));
        
                let post_exec_output = execute_shell_cmd(&mut post_exec_command)?;
        
                if !post_exec_output.status.success() {
                    let error_msg = String::from_utf8(post_exec_output.stderr)
                        .context("failed to parse post-exec stderr as UTF-8")?;
                    logger.error(format!("Post-exec command failed: {}", error_msg));
                    return Err(anyhow::anyhow!(error_msg));
                }
                
                logger.success(format!("Post-exec command {}/{} completed successfully", 
                    index + 1, 
                    post_exec_templates.len()
                ));
            }
            logger.success("All commands completed successfully");
        } else {
            logger.success("Main command completed successfully");
        }

        let json_output = serde_json::from_str("NULL")?;
        Ok(json_output)
    }
}

fn execute_shell_cmd(command_str: &mut String) -> Result<Output> {
    // Execute the command (Linux)
    #[cfg(target_os = "linux")]
    let output = Command::new("sh").arg("-c").arg(&command_str).output()?;

    // Execute the command (MacOSX)
    #[cfg(target_os = "macos")]
    let output = Command::new("sh").arg("-c").arg(&command_str).output()?;

    // Execute the command (Windows)
    #[cfg(target_os = "windows")]
    let output = Command::new("cmd").arg("/C").arg(&command_str).output()?;

    return Ok(output);
}

fn replace_template_params(params: &Map<String, Value>, command_str: &mut String) -> String {
    // Iterate through parameters and perform replacements
    for (key, value) in params {
        let placeholder = format!("{{{}}}", key); // Creates {key} format 
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

// Helper trait to handle special types
pub trait TemplateValue {
    fn to_template_string(&self) -> String;
}

// Implement for HashMap to handle networks
impl<K: ToString, V: ToString> TemplateValue for HashMap<K, V> {
    fn to_template_string(&self) -> String {
        self.iter()
            .map(|(k, v)| format!("{}={}", k.to_string(), v.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CpiCommandType {
    #[serde(rename = "create_vm")]
    CreateVM {
        guest_id: String,
        memory_mb: i32,
        os_type: String,
        resource_pool: String,
        datastore: String,
        vm_name: String,
        cpu_count: i32,
    },
    #[serde(rename = "delete_vm")]
    DeleteVM {
        vm_name: String,
    },
    #[serde(rename = "has_vm")]
    HasVM {
        vm_name: String,
    },
    ConfigureNetworks {
        vm_name: String,
        network_index: i32,
        network_type: String,
    },
    CreateDisk {
        size_mb: i32,
        disk_path: String,
    },
    AttachDisk{
        vm_name: String,
        controller_name: String,
        port: i32,
        disk_path: String,
    },
    DeleteDisk{
        vm_name: String,
        disk_path: String,
    },
    DetachDisk{
        vm_name: String,
        controller_name: String,
        port: i32,
    },
    HasDisk{
        vm_name: String,
        disk_path: String,
    },
    #[serde(rename = "set_vm_metadata")]
    SetVMMetadata{
        vm_name: String,
        key: String,
        value: String,
    },
    CreateSnapshot{
        vm_name: String,
        snapshot_name: String,
    },
    DeleteSnapshot{
        vm_name: String,
        snapshot_name: String,
    },
    HasSnapshot{
        vm_name: String,
        snapshot_name: String,
    },
    GetDisks{
        vm_name: String,
    },
    GetVM{
        vm_name: String,
    },
    RebootVM{
        vm_name: String,
    },
    SnapshotDisk{
        disk_path: String,
        snapshot_name: String,
    },
    GetSnapshots{
        vm_name: String,
    },
}

impl ToString for CpiCommandType {
    fn to_string(&self) -> String {
        match self {
            CpiCommandType::CreateVM { .. } => "create_vm".to_string(),
            CpiCommandType::DeleteVM { .. } => "delete_vm".to_string(),
            CpiCommandType::HasVM { .. } => "has_vm".to_string(),
            CpiCommandType::ConfigureNetworks { .. } => "configure_networks".to_string(),
            CpiCommandType::CreateDisk { .. } => "create_disk".to_string(),
            CpiCommandType::AttachDisk { .. } => "attach_disk".to_string(),
            CpiCommandType::DeleteDisk { .. } => "delete_disk".to_string(),
            CpiCommandType::DetachDisk { .. } => "detach_disk".to_string(),
            CpiCommandType::HasDisk { .. } => "has_disk".to_string(),
            CpiCommandType::SetVMMetadata { .. } => "set_vm_metadata".to_string(),
            CpiCommandType::CreateSnapshot { .. } => "create_snapshot".to_string(),
            CpiCommandType::DeleteSnapshot { .. } => "delete_snapshot".to_string(),
            CpiCommandType::HasSnapshot { .. } => "has_snapshot".to_string(),
            CpiCommandType::GetDisks { .. } => "get_disks".to_string(),
            CpiCommandType::GetVM { .. } => "get_vm".to_string(),
            CpiCommandType::RebootVM { .. } => "reboot_vm".to_string(),
            CpiCommandType::SnapshotDisk { .. } => "snapshot_disk".to_string(),
            CpiCommandType::GetSnapshots { .. } => "get_snapshots".to_string(),
        }
    }
}

// Return types for the API calls
#[derive(Debug, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub state: String,
    pub instance_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Volume {
    pub id: String,
    pub size: i32,
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub state: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub key: String,
    pub value: String,
}

pub struct CpiApi {
    cmd: CpiCommand,
}

pub fn test() {
    let cpi = CpiCommand::new().unwrap();
    let vm = cpi.execute(CpiCommandType::CreateVM {
        guest_id: "ubuntu".to_string(),
        memory_mb: 4096,
        cpu_count: 8,
        os_type: "Linux".to_string(),
        resource_pool: "default".to_string(),
        datastore: "datastore1".to_string(),
        vm_name: "test-vm".to_string(),
    });
    println!("Created VM: {:?}", vm);

    dprintln!("VM exists: {:?}", vm);

    // Configure networks for the VM
    let network_config = cpi.execute(CpiCommandType::ConfigureNetworks {
        vm_name: "test-vm".to_string(),
        network_index: 0,
        network_type: "VM Network".to_string(),
    });
    println!("Configured Networks: {:?}", network_config);
    
    // Set metadata for the VM
    let metadata = cpi.execute(CpiCommandType::SetVMMetadata {
        vm_name: "test-vm".to_string(),
        key: "environment".to_string(),
        value: "development".to_string(),
    });
    println!("Set VM Metadata: {:?}", metadata);
    
    // Create a disk for the VM
    let disk = cpi.execute(CpiCommandType::CreateDisk {
        size_mb: 10240,
        disk_path: "/vmfs/volumes/datastore1/test-vm/test-disk.vmdk".to_string(),
    });
    println!("Created Disk: {:?}", disk);
    
    // Attach the disk to the VM
    let attach_disk = cpi.execute(CpiCommandType::AttachDisk {
        vm_name: "test-vm".to_string(),
        controller_name: "SCSI Controller 0".to_string(),
        port: 0,
        disk_path: "/vmfs/volumes/datastore1/test-vm/test-disk.vmdk".to_string(),
    });
    println!("Attached Disk: {:?}", attach_disk);
}
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use anyhow::{Context, Result};
use ez_logging::println;
use colored::Colorize;

use super::parsers::{OutputParser, PostExecCommand, parse_command_output};
use super::utils::{execute_shell_cmd, replace_template_params};

/// Execute a sequence of commands and handle their output.
///
/// This macro simplifies the execution of multiple commands and standardizes 
/// error handling across command sequences. It iterates through the provided commands,
/// executes each one, and handles output parsing based on the command configuration.
///
/// # Parameters
///
/// * `$params` - A reference to a `Map<String, Value>` containing template parameters
///   that will be used to replace placeholders in the command string.
/// * `$commands` - A reference to a Vec or slice of `PostExecCommand` objects to execute.
/// * `$error_context` - A string literal providing context for error messages (e.g., "pre-attach", "post-exec").
///
/// # Error Handling
///
/// The macro will:
/// 1. Replace template parameters in the command string
/// 2. Execute the command using `execute_shell_cmd`
/// 3. If an output parser is provided, parse the command output using it
/// 4. If no parser is provided and the command fails, return an error with the stderr content
///
/// # Example
///
/// ```rust
/// // Execute pre-attach commands
/// execute_command_sequence!(params, &pre_attach_commands, "pre-attach");
///
/// // Execute post-execution commands
/// execute_command_sequence!(params, &post_exec_commands, "post-exec");
/// ```
///
/// # Implementation Details
///
/// The macro expands to a `for` loop that processes each command and applies consistent
/// error handling logic. This centralizes command execution logic and ensures consistent
/// behavior across different command sequences.
macro_rules! execute_command_sequence {
    ($params:expr, $commands:expr, $error_context:expr) => {
        for cmd in $commands {
            let mut cmd_str = replace_template_params($params, &mut cmd.command.clone());
            let output = execute_shell_cmd(&mut cmd_str)?;
            
            if let Some(ref parser) = cmd.output_parser {
                parse_command_output(&output, parser)?;
            } else if !output.status.success() {
                let error_msg = String::from_utf8(output.stderr.clone())
                    .context(format!("failed to parse {} stderr as UTF-8", $error_context))?;
                return Err(anyhow::anyhow!(error_msg));
            }
        }
    };
}

/// Declare all command types with their parameters in a concise, declarative style.
///
/// This macro generates both the `CpiCommandType` enum and its `ToString` implementation
/// from a single, readable declaration. It dramatically reduces boilerplate and keeps
/// command definitions centralized and consistent.
///
/// # Syntax
///
/// ```
/// declare_command_types! {
///     VariantName "serialized_name" {
///         field_name: Type,
///         another_field: AnotherType
///     },
///     
///     AnotherVariant "another_name" {}  // Empty braces required for no-field variants
/// }
/// ```
///
/// # Generated Code
///
/// The macro expands to:
///
/// 1. A complete enum definition with:
///    - Debug, Serialize, Deserialize, and Clone derives
///    - snake_case renaming for all variants
///    - Specific rename attributes for each variant
///    - Struct-like variants with specified fields
///
/// 2. A ToString implementation that:
///    - Maps each enum variant to its serialized name
///    - Handles variants with fields using pattern matching
///
/// # Parameters
///
/// * `$variant` - The Rust identifier for the enum variant (e.g., `CreateVm`)
/// * `$rename` - A string literal for the serialized name (e.g., `"create_vm"`)
/// * `$field` - Field names for struct-like variants (e.g., `vm_name`)
/// * `$type` - Rust types for each field (e.g., `String`, `u32`)
///
/// # Example
///
/// ```rust
/// declare_command_types! {
///     TestInstall "test_install" {},
///     
///     CreateVm "create_vm" {
///         vm_name: String,
///         os_type: String,
///         memory_mb: u32,
///         cpu_count: u32
///     },
///     
///     DeleteVm "delete_vm" {
///         vm_name: String
///     }
/// }
/// ```
///
/// # Important Usage Note
///
/// All variants, including those without fields, must be instantiated with braces:
/// - `CpiCommandType::TestInstall {}` (not just `CpiCommandType::TestInstall`)
/// - `CpiCommandType::CreateVm { vm_name: "test".to_string(), ... }`
///
/// # Benefits
///
/// * **Maintainability**: Adding or modifying commands requires changing only one place
/// * **Consistency**: Ensures serialized names always match in both enum definition and ToString impl
/// * **Readability**: Presents command structure in a clean, tabular format
/// * **Error Prevention**: Reduces the chance of mismatches between enum variants and their string representations
///
/// # Implementation Notes
///
/// The macro uses nested repetition to handle both the list of variants and the list of fields
/// within each variant. Empty field lists are supported for commands that don't require parameters.
macro_rules! declare_command_types {
    (
        $(
            $variant:ident $rename:literal {
                $(
                    $field:ident: $type:ty
                ),*
            }
        ),*
    ) => {
        #[derive(Debug, Serialize, Deserialize, Clone)]
        #[serde(rename_all = "snake_case")]
        pub enum CpiCommandType {
            $(
                #[serde(rename = $rename)]
                $variant {
                    $(
                        $field: $type,
                    )*
                },
            )*
        }

        impl ToString for CpiCommandType {
            fn to_string(&self) -> String {
                match self {
                    $(
                        CpiCommandType::$variant { .. } => $rename.to_string(),
                    )*
                }
            }
        }
    }
}

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
        execute_command_sequence!(params, &pre_attach_commands, "pre-attach");

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
        execute_command_sequence!(params, &post_exec_commands, "post-exec");

        Ok(result)
    }
}

// Define command types using the macro
declare_command_types! {
    TestInstall "test_install" {},
    
    CreateVm "create_vm" {
        vm_name: String,
        os_type: String,
        memory_mb: u32,
        cpu_count: u32
    },
    
    DeleteVm "delete_vm" {
        vm_name: String
    },
    
    HasVm "has_vm" {
        vm_id: String
    },
    
    StartVm "start_vm" {
        vm_name: String
    },
    
    ConfigureNetworks "configure_networks" {
        vm_name: String,
        network_index: u32,
        network_type: String
    },
    
    CreateDisk "create_disk" {
        disk_path: String,
        size_mb: u64
    },
    
    DeleteDisk "delete_disk" {
        disk_path: String
    },
    
    AttachDisk "attach_disk" {
        vm_name: String,
        port: u32,
        disk_path: String,
        controller_name: String
    },
    
    DetachDisk "detach_disk" {
        vm_name: String,
        controller_name: String,
        port: u32
    },
    
    HasDisk "has_disk" {
        disk_path: String
    },
    
    SetVmMetadata "set_vm_metadata" {
        vm_name: String,
        key: String,
        value: String
    },
    
    CreateSnapshot "create_snapshot" {
        vm_name: String,
        snapshot_name: String
    },
    
    DeleteSnapshot "delete_snapshot" {
        vm_name: String,
        snapshot_name: String
    },
    
    HasSnapshot "has_snapshot" {
        vm_name: String,
        snapshot_name: String
    },
    
    GetDisks "get_disks" {},
    
    GetVm "get_vm" {
        vm_name: String
    },
    
    RebootVm "reboot_vm" {
        vm_name: String
    },
    
    SnapshotDisk "snapshot_disk" {
        source_disk_path: String,
        target_disk_path: String
    },
    
    GetSnapshots "get_snapshots" {
        vm_name: String
    }
}
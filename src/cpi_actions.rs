use serde::{ Deserialize, Serialize };
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::process::Command;
use std::fs;

pub struct CpiCommand {
    pub config: String,
}

impl CpiCommand {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let config_str = fs::read_to_string("./CPIs/cpi-vb.json")?;
        let config_json: Value = serde_json::from_str(&config_str)?;

        Ok(Self {
            config: config_json.to_string(),
        })
    }

    pub fn execute(&self, command: CpiCommandType) -> Result<Value, Box<dyn Error>> {
        let config_json: Value = serde_json::from_str(&self.config)?;
        let actions = config_json.get("actions").ok_or("actions not found")?;
        let command_type = actions.get(command.to_string()).ok_or("command type not found")?;
        let command_template = command_type
            .get("command")
            .ok_or("command not found")?
            .as_str()
            .unwrap();
    
        // Serialize the enum variant to a JSON Value
        let params: Value = serde_json::to_value(&command)?;
        
        // Extract the inner object from the enum variant
        let params = params.as_object()
            .and_then(|obj| obj.values().next())
            .and_then(|v| v.as_object())
            .unwrap_or_else(|| panic!("Failed to get params from command"));

        println!("Params: {:?}", params);
    
        // Replace all parameters in the template
        let mut command_str = command_template.to_string();
        
        // Iterate through parameters and perform replacements
        for (key, value) in params {
            let placeholder = format!("{{{}}}", key);  // Creates {key} format
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
        
            command_str = command_str.replace(&placeholder, &replacement);
        }

        println!("Executing command: {}", command_str);
    
        // Execute the command
        let output = Command::new("sh")
            .arg("-c")
            .arg(&command_str)
            .output()?;
    
        if output.status.success() {
            // Parse the output as JSON and return
            let output_str = String::from_utf8(output.stdout)?;
            let json_output: Value = serde_json::from_str(&output_str)?;
            Ok(json_output)
        } else {
            let error_msg = String::from_utf8(output.stderr)?;
            Err(error_msg.into())
        }
    }
    
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
    CreateVM {
        guest_id: String,
        memory_mb: i32,
        num_cpus: i32,
        resource_pool: String,
        datastore: String,
        vm_name: String,
    },
    DeleteVM {
        vm_id: String,
    },
    HasVM {
        vm_id: String,
    },
    ConfigureNetworks {
        vm_id: String,
        networks: HashMap<String, String>,
        net_adapter: String,
    },
    CreateDisk {
        size_gb: i32,
        datastore: String,
        disk_type: String,
        iops: i32,
        thin_provisioned: bool,
        kms_key_id: String,
    },
    AttachDisk(),
    DetachDisk(),
    HasDisk(),
    SetVMMetadata(),
    CreateSnapshot(),
    DeleteSnapshot(),
    HasSnapshot(),
    GetDisks(),
    GetVM(),
    RebootVM(),
    SnapshotDisk(),
    GetSnapshots(),
}

impl ToString for CpiCommandType {
    fn to_string(&self) -> String {
        match self {
            CpiCommandType::CreateVM { .. } => "create_vm".to_string(),
            CpiCommandType::DeleteVM { .. } => "delete_vm".to_string(),
            CpiCommandType::HasVM{ .. } => "has_vm".to_string(),
            CpiCommandType::ConfigureNetworks{ .. } => "configure_networks".to_string(),
            CpiCommandType::CreateDisk{ .. } => "create_disk".to_string(),
            CpiCommandType::AttachDisk{ .. } => "attach_disk".to_string(),
            CpiCommandType::DetachDisk{ .. } => "detach_disk".to_string(),
            CpiCommandType::HasDisk{ .. } => "has_disk".to_string(),
            CpiCommandType::SetVMMetadata{ .. } => "set_vm_metadata".to_string(),
            CpiCommandType::CreateSnapshot{ .. } => "create_snapshot".to_string(),
            CpiCommandType::DeleteSnapshot{ .. } => "delete_snapshot".to_string(),
            CpiCommandType::HasSnapshot{ .. } => "has_snapshot".to_string(),
            CpiCommandType::GetDisks{ .. } => "get_disks".to_string(),
            CpiCommandType::GetVM{ .. } => "get_vm".to_string(),
            CpiCommandType::RebootVM{ .. } => "reboot_vm".to_string(),
            CpiCommandType::SnapshotDisk{ .. } => "snapshot_disk".to_string(),
            CpiCommandType::GetSnapshots{ .. } => "get_snapshots".to_string(),
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
    // let vm = cpi.execute(CpiCommandType::CreateVM {
    //     guest_id: "ubuntu".to_string(),
    //     memory_mb: 1024,
    //     num_cpus: 2,
    //     resource_pool: "default".to_string(),
    //     datastore: "datastore1".to_string(),
    //     vm_name: "test-vm".to_string(),
    // }).unwrap();
    // println!("Created VM: {:?}", vm);

    let vm = cpi.execute(CpiCommandType::HasVM {
        vm_id: "test-vm".to_string(),
    }).unwrap_err();

    println!("VM exists: {:?}", vm);
}
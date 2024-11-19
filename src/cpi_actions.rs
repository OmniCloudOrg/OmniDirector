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
        let config_str = fs::read_to_string("/CPIs/cpi-vb.json")?;
        let config_json: Value = serde_json::from_str(&config_str)?;

        Ok(Self {
            config: config_json.to_string(),
        })
    }

    pub fn execute(&self, Command: CpiCommandType) -> Result<Value, Box<dyn Error>> {
        let config_json: Value = serde_json::from_str(&self.config)?;
        let actions = config_json.get("actions").ok_or("actions not found")?;
        let command_type = actions.get(Command.to_string()).ok_or("command type not found")?;
        let command_template = command_type
            .get("command")
            .ok_or("command not found")?
            .as_str()
            .unwrap();

        // Do this for all params to fill in the command template with the enum params
        // "govc vm.create -g {guest_id} -m {memory_mb} -c {num_cpus} -pool {resource_pool} -ds {datastore} {vm_name}"
        let command = command_template.replace("{{vm_id}}", "i-12345678");

        Command::new("");

        Ok(Value::Null) // Placeholder assume command success
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
    GetSnapshots(),
}

impl ToString for CpiCommandType {
    fn to_string(&self) -> String {
        match self {
            CpiCommandType::CreateVM() => "create_vm".to_string(),
            CpiCommandType::DeleteVM() => "delete_vm".to_string(),
            CpiCommandType::HasVM() => "has_vm".to_string(),
            CpiCommandType::ConfigureNetworks() => "configure_networks".to_string(),
            CpiCommandType::CreateDisk() => "create_disk".to_string(),
            CpiCommandType::AttachDisk() => "attach_disk".to_string(),
            CpiCommandType::DetachDisk() => "detach_disk".to_string(),
            CpiCommandType::HasDisk() => "has_disk".to_string(),
            CpiCommandType::SetVMMetadata() => "set_vm_metadata".to_string(),
            CpiCommandType::CreateSnapshot() => "create_snapshot".to_string(),
            CpiCommandType::DeleteSnapshot() => "delete_snapshot".to_string(),
            CpiCommandType::HasSnapshot() => "has_snapshot".to_string(),
            CpiCommandType::GetDisks() => "get_disks".to_string(),
            CpiCommandType::GetVM() => "get_vm".to_string(),
            CpiCommandType::RebootVM() => "reboot_vm".to_string(),
            CpiCommandType::SnapshotDisk() => "snapshot_disk".to_string(),
            CpiCommandType::GetSnapshots() => "get_snapshots".to_string(),
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

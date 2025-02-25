use super::prelude::*;
use anyhow::Error;
use serde_json::{Value, json};
use serde::{Serialize,Deserialize};
use std::collections::HashMap;

macro_rules! define_cpi_actions {
    (
        $(
            $action:ident {
                $(
                    $param_name:ident: $param_type:ty
                ),*
            } => $handler_fn:ident($($handler_param:ident),*);
        )*
    ) => {
        #[derive(Serialize, Deserialize, Debug)]
        #[serde(rename_all = "snake_case")]
        pub enum CpiAction {
            $(
                $action {
                    $(
                        $param_name: $param_type,
                    )*
                },
            )*
        }

        impl CpiAction {
            pub fn execute(&self) -> Result<Value, Error> {
                match self {
                    $(
                        CpiAction::$action { $($param_name),* } => 
                            $handler_fn($($handler_param),*),
                    )*
                }
            }
        }
    };
}

// Example usage of the macro with explicitly specified parameters
define_cpi_actions! {
    TestInstall {} => test_install();
    CreateVm { 
        vm_name: String, 
        os_type: String, 
        memory_mb: u32, 
        cpu_count: u32 
    } => create_vm(vm_name, os_type, memory_mb, cpu_count);
    // DeleteVm { 
    //     vm_name: String 
    // } => delete_vm(vm_name);
    // HasVm { 
    //     vm_id: String 
    // } => has_vm(vm_id);
    // StartVm { 
    //     vm_name: String 
    // } => start_vm(vm_name);
    // ConfigureNetworks { 
    //     vm_name: String, 
    //     network_index: u32, 
    //     network_type: String 
    // } => configure_networks(vm_name, network_index, network_type);
    // CreateDisk { 
    //     disk_path: String, 
    //     size_mb: u64 
    // } => create_disk(disk_path, size_mb);
    // DeleteDisk { 
    //     disk_path: String 
    // } => delete_disk(disk_path);
    // AttachDisk { 
    //     vm_name: String, 
    //     controller_name: String, 
    //     port: u32, 
    //     disk_path: String 
    // } => attach_disk(vm_name, controller_name, port, disk_path);
    // DetachDisk { 
    //     vm_name: String, 
    //     controller_name: String, 
    //     port: u32 
    // } => detach_disk(vm_name, controller_name, port);
    // HasDisk { 
    //     disk_path: String 
    // } => has_disk(disk_path);
    // SetVmMetadata { 
    //     vm_name: String, 
    //     key: String, 
    //     value: String 
    // } => set_vm_metadata(vm_name, key, value);
    // CreateSnapshot { 
    //     vm_name: String, 
    //     snapshot_name: String 
    // } => create_snapshot(vm_name, snapshot_name);
    // DeleteSnapshot { 
    //     vm_name: String, 
    //     snapshot_name: String 
    // } => delete_snapshot(vm_name, snapshot_name);
    // HasSnapshot { 
    //     vm_name: String, 
    //     snapshot_name: String 
    // } => has_snapshot(vm_name, snapshot_name);
    // GetDisks {} => get_disks();
    // GetVm { 
    //     vm_name: String 
    // } => get_vm(vm_name);
    // RebootVm { 
    //     vm_name: String 
    // } => reboot_vm(vm_name);
    // SnapshotDisk { 
    //     source_disk_path: String, 
    //     target_disk_path: String 
    // } => snapshot_disk(source_disk_path, target_disk_path);
    // GetSnapshots { 
    //     vm_name: String 
    // } => get_snapshots(vm_name);
}

// Function declarations - compiler will check types match those in the macro
fn test_install() -> Result<Value, Error> {
    // Implementation
    Ok(json!({"status": "success"}))
}

fn create_vm(vm_name: &String, os_type: &String, memory_mb: &u32, cpu_count: &u32) -> Result<Value, Error> {
    // Implementation
    Ok(json!({
        "name": vm_name,
        "os": os_type,
        "memory": *memory_mb,
        "cpu": *cpu_count
    }))
}

fn reboot_vm(vm_name: &String) -> Result<Value, Error> {
    // Implementation
    println!("Rebooting VM: {}", vm_name);
    Ok(json!(true))
}

// Example of usage
fn example() -> Result<Value, Error> {
    // Simple action without parameters
    let test_result = CpiAction::TestInstall {}.execute()?;
    
    // Action with parameters
    let vm_result = CpiAction::CreateVm {
        vm_name: "test-vm".to_string(),
        os_type: "linux".to_string(),
        memory_mb: 2048,
        cpu_count: 2
    }.execute()?;
    
    Ok(vm_result)
}

#[derive(Deserialize, Debug)]
pub struct Cpi {
    pub name: String,
    pub mode: String,
    pub version: String,
    pub description: String,
    pub actions: HashMap<String, CpiAction>,
}
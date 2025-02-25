use anyhow::{Context, Result};
use serde_json;

use super::commands::{CpiCommand, CpiCommandType};
use super::models::{VirtualMachine, VirtualDisk, Snapshot};

/// High-level API for CPI operations
pub struct CpiApi {
    cmd: CpiCommand,
}

impl CpiApi {
    /// Create a new CpiApi instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            cmd: CpiCommand::new()?,
        })
    }

    /// Test if VirtualBox is installed and get its version
    pub fn test_install(&self) -> Result<String> {
        // Note the empty braces {} for struct-variants with no fields
        let result = self.cmd.execute(CpiCommandType::TestInstall {})?;
        Ok(result.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string())
    }

    /// Create a new virtual machine
    pub fn create_vm(
        &self,
        name: &str,
        os_type: &str,
        memory_mb: u32,
        cpu_count: u32,
    ) -> Result<VirtualMachine> {
        let _ = self.cmd.execute(CpiCommandType::CreateVm {
            vm_name: name.to_string(),
            os_type: os_type.to_string(),
            memory_mb,
            cpu_count,
        })?;

        // Return the created VM info
        let vm_info = self.get_vm(name)?;
        Ok(vm_info)
    }

    /// Delete a virtual machine
    pub fn delete_vm(&self, name: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::DeleteVm {
            vm_name: name.to_string(),
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Check if a VM exists
    pub fn has_vm(&self, vm_id: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::HasVm {
            vm_id: vm_id.to_string(),
        })?;
        
        Ok(result.get("exists")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Start a virtual machine
    pub fn start_vm(&self, name: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::StartVm {
            vm_name: name.to_string(),
        })?;

        Ok(result.get("started_vm")
            .and_then(|v| v.as_str())
            .map(|s| s == name)
            .unwrap_or(false))
    }

    /// Reboot a virtual machine
    pub fn reboot_vm(&self, name: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::RebootVm {
            vm_name: name.to_string(),
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Configure network settings for a VM
    pub fn configure_networks(&self, vm_name: &str, network_index: u32, network_type: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::ConfigureNetworks {
            vm_name: vm_name.to_string(),
            network_index,
            network_type: network_type.to_string(),
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Get information about a specific VM
    pub fn get_vm(&self, name: &str) -> Result<VirtualMachine> {
        let result = self.cmd.execute(CpiCommandType::GetVm {
            vm_name: name.to_string(),
        })?;

        let vm: VirtualMachine = serde_json::from_value(result)
            .context("failed to parse VM info")?;
        Ok(vm)
    }

    /// Create a new virtual disk
    pub fn create_disk(&self, path: &str, size_mb: u64) -> Result<VirtualDisk> {
        let result = self.cmd.execute(CpiCommandType::CreateDisk {
            disk_path: path.to_string(),
            size_mb,
        })?;

        let disk: VirtualDisk = serde_json::from_value(result)
            .context("failed to parse disk info")?;
        Ok(disk)
    }

    /// Delete a virtual disk
    pub fn delete_disk(&self, path: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::DeleteDisk {
            disk_path: path.to_string(),
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Check if a disk exists
    pub fn has_disk(&self, path: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::HasDisk {
            disk_path: path.to_string(),
        })?;
        
        Ok(result.get("exists")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Get a list of all virtual disks
    pub fn get_disks(&self) -> Result<Vec<VirtualDisk>> {
        // Note the empty braces {} for struct-variants with no fields
        let result = self.cmd.execute(CpiCommandType::GetDisks {})?;
        
        let disks: Vec<VirtualDisk> = serde_json::from_value(result)
            .context("failed to parse disk list")?;
        Ok(disks)
    }

    /// Attach a disk to a VM
    pub fn attach_disk(&self, vm_name: &str, port: u32, disk_path: &str, controller_name: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::AttachDisk {
            vm_name: vm_name.to_string(),
            port,
            disk_path: disk_path.to_string(),
            controller_name: controller_name.to_string(),
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Detach a disk from a VM
    pub fn detach_disk(&self, vm_name: &str, controller_name: &str, port: u32) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::DetachDisk {
            vm_name: vm_name.to_string(),
            controller_name: controller_name.to_string(),
            port,
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Set metadata on a VM
    pub fn set_vm_metadata(&self, vm_name: &str, key: &str, value: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::SetVmMetadata {
            vm_name: vm_name.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Create a snapshot of a VM
    pub fn create_snapshot(&self, vm_name: &str, snapshot_name: &str) -> Result<Snapshot> {
        let result = self.cmd.execute(CpiCommandType::CreateSnapshot {
            vm_name: vm_name.to_string(),
            snapshot_name: snapshot_name.to_string(),
        })?;

        let snapshot: Snapshot = serde_json::from_value(result)
            .context("failed to parse snapshot info")?;
        Ok(snapshot)
    }

    /// Delete a VM snapshot
    pub fn delete_snapshot(&self, vm_name: &str, snapshot_name: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::DeleteSnapshot {
            vm_name: vm_name.to_string(),
            snapshot_name: snapshot_name.to_string(),
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Check if a snapshot exists
    pub fn has_snapshot(&self, vm_name: &str, snapshot_name: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::HasSnapshot {
            vm_name: vm_name.to_string(),
            snapshot_name: snapshot_name.to_string(),
        })?;
        
        Ok(result.get("exists")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Get a list of all snapshots for a VM
    pub fn get_snapshots(&self, vm_name: &str) -> Result<Vec<Snapshot>> {
        let result = self.cmd.execute(CpiCommandType::GetSnapshots {
            vm_name: vm_name.to_string(),
        })?;

        let snapshots: Vec<Snapshot> = serde_json::from_value(result)
            .context("failed to parse snapshot list")?;
        Ok(snapshots)
    }
    
    /// Create a snapshot of a disk
    pub fn snapshot_disk(&self, source_disk_path: &str, target_disk_path: &str) -> Result<bool> {
        let result = self.cmd.execute(CpiCommandType::SnapshotDisk {
            source_disk_path: source_disk_path.to_string(),
            target_disk_path: target_disk_path.to_string(),
        })?;
        
        Ok(result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }
}

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
        let _disk = api.create_disk("/tmp/test-disk.vdi", 10240)?;
        
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
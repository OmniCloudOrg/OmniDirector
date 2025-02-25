use serde::{Deserialize, Serialize};

/// Represents a virtual machine in the VirtualBox environment
#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualMachine {
    pub name:      String,
    pub uuid:      String,
    pub state:     String,
    pub os_type:   String,
    pub memory_mb: u32,
    pub cpu_count: u32,
}

/// Represents a virtual disk in the VirtualBox environment
#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualDisk {
    pub uuid:     String,
    pub location: String,
    pub state:    String,
    pub size_mb:  u64,
}

/// Represents a VM snapshot in the VirtualBox environment
#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub name:      String,
    pub uuid:      String,
    pub timestamp: String,
}

/// Generic result structure for command operations
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult<T> {
    pub success: bool,
    pub data:    Option<T>,
    pub error:   Option<String>,
}
use serde::{Deserialize, Serialize};

#[derive(Debug,Serialize,Deserialize)]


struct CPi {
    name: String,
    #[serde(rename = "type")]
    cpi_type: String,
    actions: CpiActions 
}
#[derive(Debug,Serialize,Deserialize)]
struct CpiActions {
    pub test_install: Action,
    pub create_vm: Action,
    pub delete_vm: Action,
    pub has_vm: Action,
    pub configure_networks: Action,
    pub create_disk: Action,
    pub delete_disk: Action,
    pub attach_disk: Action,
    pub detach_disk: Action,
    pub has_disk: Action,
    pub set_vm_metadata: Action,
    pub create_snapshot: Action,
    pub delete_snapshot: Action,
    pub has_snapshot: Action,
    pub get_disks: Action,
    pub get_vm: Action,
    pub reboot_vm: Action,
    pub snapshot_disk: Action,
    pub get_snapshots: Action,
}
#[derive(Debug,Serialize,Deserialize)]

struct Action {
    command: String,
    pub params: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_exec: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_attach: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_output: Option<String>,
}   



// impl Default for CPi {
//     fn default() -> Self { 
//         Self { name: Default::default(), cpi_type: Default::default(), actions: Default::default() }
//     }
// }

use crate::cpi_actions::*;

//-----------------------------------------------------------------------------
// Unit tests for the CPI Interface Functions
//-----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_create_vm() {
        let api = CpiApi::new("{}").unwrap();
        let instance = api
            .create_vm(
                "ami-12345678",
                "t2.micro",
                "subnet-12345678",
                Some(vec!["sg-12345678"]),
                Some("key-pair"),
                Some("user-data"),
                Some("iam-role"),
                Some("vm-name")
            )
            .unwrap();
        assert_eq!(instance.id, "");
        assert_eq!(instance.state, "");
        assert_eq!(instance.instance_type, "");
    }

    #[test]
    fn test_delete_vm() {
        let api = CpiApi::new("{}").unwrap();
        let result = api.delete_vm("i-12345678");
        assert!(result.is_ok());
    }

    #[test]
    fn test_has_vm() {
        let api = CpiApi::new("{}").unwrap();
        let result = api.has_vm("i-12345678").unwrap();
        assert!(result);
    }

    #[test]
    fn test_create_disk() {
        let api = CpiApi::new("{}").unwrap();
        let volume = api
            .create_disk(100, "us-west-2a", Some("gp2"), Some(3000), Some(true), Some("kms-key-id"))
            .unwrap();
        assert_eq!(volume.id, "");
        assert_eq!(volume.size, 0);
        assert_eq!(volume.state, "");
    }

    #[test]
    fn test_delete_disk() {
        let api = CpiApi::new("{}").unwrap();
        let result = api.delete_disk("vol-12345678");
        assert!(result.is_ok());
    }

    #[test]
    fn test_attach_disk() {
        let api = CpiApi::new("{}").unwrap();
        let result = api.attach_disk("vol-12345678", "i-12345678", "/dev/sdf");
        assert!(result.is_ok());
    }

    #[test]
    fn test_detach_disk() {
        let api = CpiApi::new("{}").unwrap();
        let result = api.detach_disk("vol-12345678");
        assert!(result.is_ok());
    }

    #[test]
    fn test_has_disk() {
        let api = CpiApi::new("{}").unwrap();
        let result = api.has_disk("vol-12345678").unwrap();
        assert!(result);
    }

    #[test]
    fn test_set_vm_metadata() {
        let api = CpiApi::new("{}").unwrap();
        let tag = api.set_vm_metadata("i-12345678", "Name", "TestVM").unwrap();
        assert_eq!(tag.key, "Name");
        assert_eq!(tag.value, "TestVM");
    }

    #[test]
    fn test_create_snapshot() {
        let api = CpiApi::new("{}").unwrap();
        let snapshot = api.create_snapshot("vol-12345678", "Test snapshot").unwrap();
        assert_eq!(snapshot.id, "");
        assert_eq!(snapshot.state, "");
        assert_eq!(snapshot.description, "Test snapshot");
    }

    #[test]
    fn test_delete_snapshot() {
        let api = CpiApi::new("{}").unwrap();
        let result = api.delete_snapshot("snap-12345678");
        assert!(result.is_ok());
    }

    #[test]
    fn test_has_snapshot() {
        let api = CpiApi::new("{}").unwrap();
        let result = api.has_snapshot("snap-12345678").unwrap();
        assert!(result);
    }

    #[test]
    fn test_get_disks() {
        let api = CpiApi::new("{}").unwrap();
        let volumes = api.get_disks("owner-id").unwrap();
        assert!(volumes.is_empty());
    }

    #[test]
    fn test_get_vm() {
        let api = CpiApi::new("{}").unwrap();
        let instance = api.get_vm("i-12345678").unwrap();
        assert_eq!(instance.id, "");
        assert_eq!(instance.state, "");
        assert_eq!(instance.instance_type, "");
    }

    #[test]
    fn test_reboot_vm() {
        let api = CpiApi::new("{}").unwrap();
        let result = api.reboot_vm("i-12345678");
        assert!(result.is_ok());
    }

    #[test]
    fn test_snapshot_disk() {
        let api = CpiApi::new("{}").unwrap();
        let snapshot = api.snapshot_disk("vol-12345678", "Test snapshot").unwrap();
        assert_eq!(snapshot.id, "");
        assert_eq!(snapshot.state, "");
        assert_eq!(snapshot.description, "Test snapshot");
    }

    #[test]
    fn test_get_snapshots() {
        let api = CpiApi::new("{}").unwrap();
        let snapshots = api.get_snapshots("owner-id").unwrap();
        assert!(snapshots.is_empty());
    }
}

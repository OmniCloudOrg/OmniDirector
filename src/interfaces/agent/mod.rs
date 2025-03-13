use reqwest::{Client, Error as ReqwestError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use libomni::cpi::container::CpiCommandType;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] ReqwestError),
    #[error("Server returned error: {0}")]
    ServerError(String),
    #[error("Failed to parse response: {0}")]
    ParseError(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct ContainerClient {
    client: Client,
    base_url: String,
}

impl ContainerClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
        }
    }

    pub async fn create_container(
        &self,
        guest_id: impl Into<String>,
        memory_mb: i32,
        os_type: impl Into<String>,
        resource_pool: impl Into<String>,
        datastore: impl Into<String>,
        container_name: impl Into<String>,
        cpu_count: i32,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::CreateContainer {
            guest_id: guest_id.into(),
            memory_mb,
            os_type: os_type.into(),
            resource_pool: resource_pool.into(),
            datastore: datastore.into(),
            container_name: container_name.into(),
            cpu_count,
        };
        self.execute_command("/containers/create", command).await
    }

    pub async fn start_container(&self, container_name: impl Into<String>) -> Result<String, ClientError> {
        let command = CpiCommandType::StartContainer {
            container_name: container_name.into(),
        };
        self.execute_command("/containers/start", command).await
    }

    pub async fn delete_container(&self, container_name: impl Into<String>) -> Result<String, ClientError> {
        let command = CpiCommandType::DeleteContainer {
            container_name: container_name.into(),
        };
        self.execute_command("/containers/delete", command).await
    }

    pub async fn has_container(&self, container_name: impl Into<String>) -> Result<String, ClientError> {
        let command = CpiCommandType::HasContainer {
            container_name: container_name.into(),
        };
        self.execute_command("/containers/has", command).await
    }

    pub async fn configure_networks(
        &self,
        container_name: impl Into<String>,
        network_index: i32,
        network_type: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::ConfigureNetworks {
            container_name: container_name.into(),
            network_index,
            network_type: network_type.into(),
        };
        self.execute_command("/containers/configure_networks", command).await
    }

    pub async fn create_volume(
        &self,
        size_mb: i32,
        volume_path: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::CreateDisk {
            size_mb,
            volume_path: volume_path.into(),
        };
        self.execute_command("/containers/create_volume", command).await
    }

    pub async fn attach_volume(
        &self,
        container_name: impl Into<String>,
        controller_name: impl Into<String>,
        port: i32,
        volume_path: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::AttachDisk {
            container_name: container_name.into(),
            controller_name: controller_name.into(),
            port,
            volume_path: volume_path.into(),
        };
        self.execute_command("/containers/attach_volume", command).await
    }

    pub async fn delete_volume(
        &self,
        container_name: impl Into<String>,
        volume_path: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::DeleteDisk {
            container_name: container_name.into(),
            volume_path: volume_path.into(),
        };
        self.execute_command("/containers/delete_volume", command).await
    }

    pub async fn detach_volume(
        &self,
        container_name: impl Into<String>,
        controller_name: impl Into<String>,
        port: i32,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::DetachDisk {
            container_name: container_name.into(),
            controller_name: controller_name.into(),
            port,
        };
        self.execute_command("/containers/detach_volume", command).await
    }

    pub async fn has_volume(
        &self,
        container_name: impl Into<String>,
        volume_path: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::HasDisk {
            container_name: container_name.into(),
            volume_path: volume_path.into(),
        };
        self.execute_command("/containers/has_volume", command).await
    }

    pub async fn set_metadata(
        &self,
        container_name: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::SetContainerMetadata {
            container_name: container_name.into(),
            key: key.into(),
            value: value.into(),
        };
        self.execute_command("/containers/set_metadata", command).await
    }

    pub async fn create_snapshot(
        &self,
        container_name: impl Into<String>,
        snapshot_name: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::CreateSnapshot {
            container_name: container_name.into(),
            snapshot_name: snapshot_name.into(),
        };
        self.execute_command("/containers/create_snapshot", command).await
    }

    pub async fn delete_snapshot(
        &self,
        container_name: impl Into<String>,
        snapshot_name: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::DeleteSnapshot {
            container_name: container_name.into(),
            snapshot_name: snapshot_name.into(),
        };
        self.execute_command("/containers/delete_snapshot", command).await
    }

    pub async fn has_snapshot(
        &self,
        container_name: impl Into<String>,
        snapshot_name: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::HasSnapshot {
            container_name: container_name.into(),
            snapshot_name: snapshot_name.into(),
        };
        self.execute_command("/containers/has_snapshot", command).await
    }

    pub async fn get_volumes(
        &self,
        container_name: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::GetDisks {
            container_name: container_name.into(),
        };
        self.execute_command("/containers/get_volumes", command).await
    }

    pub async fn get_container(
        &self,
        container_name: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::GetContainer {
            container_name: container_name.into(),
        };
        self.execute_command("/containers/get", command).await
    }

    pub async fn reboot_container(
        &self,
        container_name: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::RebootContainer {
            container_name: container_name.into(),
        };
        self.execute_command("/containers/reboot", command).await
    }

    pub async fn snapshot_volume(
        &self,
        volume_path: impl Into<String>,
        snapshot_name: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::SnapshotDisk {
            volume_path: volume_path.into(),
            snapshot_name: snapshot_name.into(),
        };
        self.execute_command("/containers/snapshot_volume", command).await
    }

    pub async fn get_snapshots(
        &self,
        container_name: impl Into<String>,
    ) -> Result<String, ClientError> {
        let command = CpiCommandType::GetSnapshots {
            container_name: container_name.into(),
        };
        self.execute_command("/containers/get_snapshots", command).await
    }

    async fn execute_command(&self, endpoint: &str, command: CpiCommandType) -> Result<String, ClientError> {
        let url = format!("{}{}", self.base_url, endpoint);
        
        let response = self.client
            .post(url)
            .json(&command)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ClientError::ServerError(
                response.text().await.unwrap_or_else(|_| "Unknown server error".to_string())
            ));
        }

        Ok(response.text().await?)
    }
}
//! # Core Server Module
//!
//! Core server functionality for the OmniDirector system.

use std::sync::Arc;
use tokio::sync::RwLock;

/// Main server state
#[derive(Debug)]
pub struct ServerState {
    /// Server configuration
    config: Arc<RwLock<ServerConfig>>,
    /// Running status
    running: Arc<RwLock<bool>>,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server port
    pub port: u16,
    /// Server host
    pub host: String,
    /// Enable debug mode
    pub debug: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "127.0.0.1".to_string(),
            debug: false,
        }
    }
}

impl ServerState {
    /// Create a new server state
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Check if server is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Set server running status
    pub async fn set_running(&self, running: bool) {
        *self.running.write().await = running;
    }

    /// Get server configuration
    pub async fn get_config(&self) -> ServerConfig {
        self.config.read().await.clone()
    }

    /// Update server configuration
    pub async fn update_config(&self, config: ServerConfig) {
        *self.config.write().await = config;
    }
}
//! # Configuration Management
//!
//! Centralized configuration for the OmniDirector system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Server configuration
    pub server: ServerConfig,
    /// Provider configuration
    pub providers: ProviderConfig,
    /// Feature configuration  
    pub features: FeatureConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host address
    pub host: String,
    /// Server port
    pub port: u16,
    /// Number of worker threads
    pub workers: usize,
    /// Request timeout in seconds
    pub timeout: u64,
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Providers directory path
    pub providers_dir: String,
    /// Auto-discovery enabled
    pub auto_discovery: bool,
    /// Provider-specific settings
    pub settings: HashMap<String, serde_json::Value>,
}

/// Feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Features directory path
    pub features_dir: String,
    /// Auto-discovery enabled
    pub auto_discovery: bool,
    /// Feature-specific settings
    pub settings: HashMap<String, serde_json::Value>,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    /// Enable timestamps
    pub timestamps: bool,
    /// Log format
    pub format: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8081,
                workers: 32,
                timeout: 30,
            },
            providers: ProviderConfig {
                providers_dir: "./providers".to_string(),
                auto_discovery: true,
                settings: HashMap::new(),
            },
            features: FeatureConfig {
                features_dir: "./features".to_string(),
                auto_discovery: true,
                settings: HashMap::new(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                timestamps: true,
                format: "human".to_string(),
            },
        }
    }
}

impl SystemConfig {
    /// Load configuration from file or environment
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Try to load from config file first
        if let Ok(content) = std::fs::read_to_string("omni-director.toml") {
            return Ok(toml::from_str(&content)?);
        }
        
        // Fall back to environment variables or defaults
        let mut config = Self::default();
        
        if let Ok(host) = std::env::var("OMNI_HOST") {
            config.server.host = host;
        }
        
        if let Ok(port) = std::env::var("OMNI_PORT") {
            config.server.port = port.parse().unwrap_or(8081);
        }
        
        if let Ok(providers_dir) = std::env::var("OMNI_PROVIDERS_DIR") {
            config.providers.providers_dir = providers_dir;
        }
        
        if let Ok(features_dir) = std::env::var("OMNI_FEATURES_DIR") {
            config.features.features_dir = features_dir;
        }
        
        Ok(config)
    }
}
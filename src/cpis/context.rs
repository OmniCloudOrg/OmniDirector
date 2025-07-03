//! # Server Context
//!
//! Provides plugins with access to core system services and capabilities.
//! This is the main interface plugins use to interact with the system.

use std::sync::Arc;
use std::fmt::Debug;
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;
use super::{EventSystem, FeatureRegistry, ArgumentManager, PluginError};

/// Server context trait that provides plugins with system access
#[async_trait]
pub trait ServerContext: Send + Sync + Debug {
    /// Get access to the event system
    fn events(&self) -> Arc<EventSystem>;
    
    /// Get the region/node identifier
    fn region_id(&self) -> &str;
    
    /// Log a message at the specified level
    fn log(&self, level: LogLevel, message: &str);
    
    /// Send data to a specific client/player
    async fn send_to_client(&self, client_id: &str, data: &[u8]) -> Result<(), ServerError>;
    
    /// Broadcast data to all clients
    async fn broadcast(&self, data: &[u8]) -> Result<(), ServerError>;
    
    /// Get access to the feature registry
    fn features(&self) -> Arc<FeatureRegistry>;
    
    /// Get access to the argument manager
    fn arguments(&self) -> Arc<ArgumentManager>;
    
    /// Execute a system command with elevated privileges
    async fn execute_system_command(&self, command: &str, args: &[&str]) -> Result<SystemCommandResult, ServerError>;
    
    /// Store persistent data for a plugin
    async fn store_plugin_data(&self, plugin_name: &str, key: &str, data: &Value) -> Result<(), ServerError>;
    
    /// Retrieve persistent data for a plugin
    async fn get_plugin_data(&self, plugin_name: &str, key: &str) -> Result<Option<Value>, ServerError>;
    
    /// Delete persistent data for a plugin
    async fn delete_plugin_data(&self, plugin_name: &str, key: &str) -> Result<(), ServerError>;
    
    /// Get system metrics
    async fn get_system_metrics(&self) -> Result<SystemMetrics, ServerError>;
    
    /// Schedule a task to run later
    async fn schedule_task(&self, delay_ms: u64, task_data: Value) -> Result<Uuid, ServerError>;
    
    /// Cancel a scheduled task
    async fn cancel_task(&self, task_id: Uuid) -> Result<(), ServerError>;
}

/// Log levels for the logging system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

/// Result of a system command execution
#[derive(Debug, Clone)]
pub struct SystemCommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub execution_time_ms: u64,
}

/// System metrics for monitoring
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub memory_total_mb: u64,
    pub disk_usage_mb: u64,
    pub disk_total_mb: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub uptime_seconds: u64,
    pub active_connections: u32,
}

/// Server errors that can occur in the context
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Client not found: {0}")]
    ClientNotFound(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("System command failed: {0}")]
    SystemCommandFailed(String),
    
    #[error("Data storage error: {0}")]
    DataStorageError(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Resource not available: {0}")]
    ResourceUnavailable(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Default implementation of ServerContext for the CPI system
#[derive(Debug)]
pub struct CpiServerContext {
    event_system: Arc<EventSystem>,
    feature_registry: Arc<FeatureRegistry>,
    argument_manager: Arc<ArgumentManager>,
    region_id: String,
    data_store: Arc<tokio::sync::RwLock<std::collections::HashMap<String, Value>>>,
    metrics: Arc<tokio::sync::RwLock<SystemMetrics>>,
}

impl CpiServerContext {
    pub fn new(
        event_system: Arc<EventSystem>,
        feature_registry: Arc<FeatureRegistry>,
        argument_manager: Arc<ArgumentManager>,
        region_id: String,
    ) -> Self {
        Self {
            event_system,
            feature_registry,
            argument_manager,
            region_id,
            data_store: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            metrics: Arc::new(tokio::sync::RwLock::new(SystemMetrics::default())),
        }
    }
}

#[async_trait]
impl ServerContext for CpiServerContext {
    fn events(&self) -> Arc<EventSystem> {
        Arc::clone(&self.event_system)
    }
    
    fn region_id(&self) -> &str {
        &self.region_id
    }
    
    fn log(&self, level: LogLevel, message: &str) {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        println!("[{}] [{}] {}", timestamp, level.as_str(), message);
    }
    
    async fn send_to_client(&self, client_id: &str, data: &[u8]) -> Result<(), ServerError> {
        // In a real implementation, this would send data over a network connection
        self.log(LogLevel::Debug, &format!("Sending {} bytes to client {}", data.len(), client_id));
        Ok(())
    }
    
    async fn broadcast(&self, data: &[u8]) -> Result<(), ServerError> {
        // In a real implementation, this would broadcast to all connected clients
        self.log(LogLevel::Debug, &format!("Broadcasting {} bytes to all clients", data.len()));
        Ok(())
    }
    
    fn features(&self) -> Arc<FeatureRegistry> {
        Arc::clone(&self.feature_registry)
    }
    
    fn arguments(&self) -> Arc<ArgumentManager> {
        Arc::clone(&self.argument_manager)
    }
    
    async fn execute_system_command(&self, command: &str, args: &[&str]) -> Result<SystemCommandResult, ServerError> {
        use tokio::process::Command;
        use std::time::Instant;
        
        let start = Instant::now();
        
        let output = Command::new(command)
            .args(args)
            .output()
            .await
            .map_err(|e| ServerError::SystemCommandFailed(format!("Failed to execute command: {}", e)))?;
        
        let execution_time_ms = start.elapsed().as_millis() as u64;
        
        Ok(SystemCommandResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            execution_time_ms,
        })
    }
    
    async fn store_plugin_data(&self, plugin_name: &str, key: &str, data: &Value) -> Result<(), ServerError> {
        let storage_key = format!("{}:{}", plugin_name, key);
        let mut store = self.data_store.write().await;
        store.insert(storage_key, data.clone());
        Ok(())
    }
    
    async fn get_plugin_data(&self, plugin_name: &str, key: &str) -> Result<Option<Value>, ServerError> {
        let storage_key = format!("{}:{}", plugin_name, key);
        let store = self.data_store.read().await;
        Ok(store.get(&storage_key).cloned())
    }
    
    async fn delete_plugin_data(&self, plugin_name: &str, key: &str) -> Result<(), ServerError> {
        let storage_key = format!("{}:{}", plugin_name, key);
        let mut store = self.data_store.write().await;
        store.remove(&storage_key);
        Ok(())
    }
    
    async fn get_system_metrics(&self) -> Result<SystemMetrics, ServerError> {
        let metrics = self.metrics.read().await;
        Ok(metrics.clone())
    }
    
    async fn schedule_task(&self, delay_ms: u64, task_data: Value) -> Result<Uuid, ServerError> {
        let task_id = Uuid::new_v4();
        self.log(LogLevel::Debug, &format!("Scheduled task {} to run in {}ms", task_id, delay_ms));
        
        // In a real implementation, this would use a proper task scheduler
        let task_id_clone = task_id;
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            println!("Executing scheduled task: {} with data: {}", task_id_clone, task_data);
        });
        
        Ok(task_id)
    }
    
    async fn cancel_task(&self, task_id: Uuid) -> Result<(), ServerError> {
        self.log(LogLevel::Debug, &format!("Cancelled task {}", task_id));
        // In a real implementation, this would cancel the actual task
        Ok(())
    }
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0,
            memory_total_mb: 8192,
            disk_usage_mb: 0,
            disk_total_mb: 500000,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            uptime_seconds: 0,
            active_connections: 0,
        }
    }
}

/// Helper context builder for creating server contexts
pub struct ServerContextBuilder {
    event_system: Option<Arc<EventSystem>>,
    feature_registry: Option<Arc<FeatureRegistry>>,
    argument_manager: Option<Arc<ArgumentManager>>,
    region_id: Option<String>,
}

impl ServerContextBuilder {
    pub fn new() -> Self {
        Self {
            event_system: None,
            feature_registry: None,
            argument_manager: None,
            region_id: None,
        }
    }
    
    pub fn with_event_system(mut self, event_system: Arc<EventSystem>) -> Self {
        self.event_system = Some(event_system);
        self
    }
    
    pub fn with_feature_registry(mut self, feature_registry: Arc<FeatureRegistry>) -> Self {
        self.feature_registry = Some(feature_registry);
        self
    }
    
    pub fn with_argument_manager(mut self, argument_manager: Arc<ArgumentManager>) -> Self {
        self.argument_manager = Some(argument_manager);
        self
    }
    
    pub fn with_region_id(mut self, region_id: String) -> Self {
        self.region_id = Some(region_id);
        self
    }
    
    pub fn build(self) -> Result<Arc<CpiServerContext>, ServerError> {
        let event_system = self.event_system
            .ok_or_else(|| ServerError::InternalError("Event system not provided".to_string()))?;
        let feature_registry = self.feature_registry
            .ok_or_else(|| ServerError::InternalError("Feature registry not provided".to_string()))?;
        let argument_manager = self.argument_manager
            .ok_or_else(|| ServerError::InternalError("Argument manager not provided".to_string()))?;
        let region_id = self.region_id
            .unwrap_or_else(|| "default".to_string());
        
        Ok(Arc::new(CpiServerContext::new(
            event_system,
            feature_registry,
            argument_manager,
            region_id,
        )))
    }
}

impl Default for ServerContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
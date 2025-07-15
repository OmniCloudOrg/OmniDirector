//! # Enhanced Plugin Executor
//!
//! Handles execution of both legacy and enum-based features.
//! Provides unified interface for all feature types.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde_json::Value;
use uuid::Uuid;

use super::{
    PluginError, FeatureActionEvent, FeatureActionCompleteEvent, 
    EventSystem, FeatureRegistry, ArgumentManager, ServerContext,
    EnhancedFeatureManager, FeatureContext
};
use super::context::LogLevel;

/// Enhanced executor that handles both legacy and enum-based features
#[derive(Debug)]
pub struct EnhancedPluginExecutor {
    event_system: Arc<EventSystem>,
    feature_registry: Arc<FeatureRegistry>,
    argument_manager: Arc<ArgumentManager>,
    enhanced_features: Arc<RwLock<EnhancedFeatureManager>>,
    pending_requests: Arc<RwLock<HashMap<Uuid, PendingRequest>>>,
    server_context: Arc<dyn ServerContext>,
}

/// Enhanced pending request with provider information
#[derive(Debug)]
pub struct PendingRequest {
    pub request_id: Uuid,
    pub provider: Option<String>,
    pub feature: String,
    pub action: String,
    pub arguments: HashMap<String, Value>,
    pub start_time: Instant,
    pub timeout: Duration,
    pub response_sender: tokio::sync::oneshot::Sender<Result<Value, PluginError>>,
}

/// Enhanced execution context
#[derive(Debug, Clone)]
pub struct EnhancedExecutionContext {
    pub request_id: Uuid,
    pub provider: Option<String>,
    pub feature: String,
    pub action: String,
    pub start_time: Instant,
    pub timeout: Duration,
}

impl EnhancedPluginExecutor {
    pub fn new(
        event_system: Arc<EventSystem>,
        feature_registry: Arc<FeatureRegistry>,
        argument_manager: Arc<ArgumentManager>,
        enhanced_features: Arc<RwLock<EnhancedFeatureManager>>,
        server_context: Arc<dyn ServerContext>,
    ) -> Self {
        Self {
            event_system,
            feature_registry,
            argument_manager,
            enhanced_features,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            server_context,
        }
    }

    /// Initialize the enhanced executor
    pub async fn initialize(&self) -> Result<(), PluginError> {
        let pending_requests = Arc::clone(&self.pending_requests);
        
        // Register handler for action completion events
        self.event_system.on_event::<FeatureActionCompleteEvent, _>(
            "feature:action:complete",
            move |event| {
                let pending_requests = Arc::clone(&pending_requests);
                tokio::spawn(async move {
                    Self::handle_action_complete(pending_requests, event).await
                });
                Ok(())
            }
        ).await
            .map_err(|e| PluginError::EventError(e.to_string()))?;

        Ok(())
    }

    /// Execute a feature action with optional provider
    pub async fn execute_action(
        &self,
        provider: Option<&str>,
        feature: &str,
        action: &str,
        arguments: HashMap<String, Value>,
        timeout: Option<Duration>,
    ) -> Result<Value, PluginError> {
        let request_id = Uuid::new_v4();
        let timeout = timeout.unwrap_or(Duration::from_secs(30));
        
        // First try enhanced features
        let enhanced_features = self.enhanced_features.read().await;
        if enhanced_features.get_available_features().contains(&feature.to_string()) {
            drop(enhanced_features); // Release the lock
            return self.execute_enhanced_feature(feature, action, arguments, request_id, timeout).await;
        }
        drop(enhanced_features);

        // Fall back to legacy execution
        self.execute_legacy_feature(provider, feature, action, arguments, request_id, timeout).await
    }

    /// Execute an enhanced (enum-based) feature
    async fn execute_enhanced_feature(
        &self,
        feature: &str,
        action: &str,
        arguments: HashMap<String, Value>,
        request_id: Uuid,
        _timeout: Duration,
    ) -> Result<Value, PluginError> {
        let context = EnhancedFeatureContextImpl::new(
            request_id.to_string(),
            feature.to_string(),
            Arc::clone(&self.argument_manager),
            Arc::clone(&self.server_context),
        );

        let enhanced_features = self.enhanced_features.read().await;
        let result = enhanced_features.execute_operation(feature, action, arguments, &context).await;
        
        result
    }

    /// Execute a legacy feature
    async fn execute_legacy_feature(
        &self,
        provider: Option<&str>,
        feature: &str,
        action: &str,
        arguments: HashMap<String, Value>,
        request_id: Uuid,
        timeout: Duration,
    ) -> Result<Value, PluginError> {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        
        let pending_request = PendingRequest {
            request_id,
            provider: provider.map(String::from),
            feature: feature.to_string(),
            action: action.to_string(),
            arguments: arguments.clone(),
            start_time: Instant::now(),
            timeout,
            response_sender,
        };

        // Store the pending request
        {
            let mut pending_requests = self.pending_requests.write().await;
            pending_requests.insert(request_id, pending_request);
        }

        // Emit the feature action event
        let event = FeatureActionEvent {
            feature: feature.to_string(),
            action: action.to_string(),
            arguments,
            request_id,
        };

        let event_key = format!("feature:{}:{}", feature, action);
        if let Err(e) = self.event_system.emit_event(&event_key, &event).await {
            // Clean up pending request
            let mut pending_requests = self.pending_requests.write().await;
            pending_requests.remove(&request_id);
            return Err(PluginError::EventError(e.to_string()));
        }

        // Wait for response with timeout
        match tokio::time::timeout(timeout, response_receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                // Clean up pending request
                let mut pending_requests = self.pending_requests.write().await;
                pending_requests.remove(&request_id);
                Err(PluginError::ExecutionFailed("Response channel closed".to_string()))
            },
            Err(_) => {
                // Clean up pending request
                let mut pending_requests = self.pending_requests.write().await;
                pending_requests.remove(&request_id);
                Err(PluginError::ExecutionFailed("Execution timeout".to_string()))
            }
        }
    }

    /// Handle action completion event
    async fn handle_action_complete(
        pending_requests: Arc<RwLock<HashMap<Uuid, PendingRequest>>>,
        event: FeatureActionCompleteEvent,
    ) {
        let mut pending_requests = pending_requests.write().await;
        
        if let Some(pending_request) = pending_requests.remove(&event.request_id) {
            let _ = pending_request.response_sender.send(event.result);
        }
    }

    /// Execute multiple actions in batch
    pub async fn execute_batch(
        &self,
        actions: Vec<(Option<String>, String, String, HashMap<String, Value>)>, // (provider, feature, action, args)
        timeout: Option<Duration>,
    ) -> Vec<Result<Value, PluginError>> {
        let mut results = Vec::new();
        
        for (provider, feature, action, args) in actions {
            let result = self.execute_action(
                provider.as_deref(),
                &feature,
                &action,
                args,
                timeout
            ).await;
            results.push(result);
        }
        
        results
    }

    /// Get execution statistics
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        let pending_requests = self.pending_requests.read().await;
        let pending_count = pending_requests.len();
        
        let mut total_wait_time = Duration::from_secs(0);
        let mut oldest_request_age = Duration::from_secs(0);
        let now = Instant::now();
        
        for request in pending_requests.values() {
            let age = now.duration_since(request.start_time);
            total_wait_time += age;
            if age > oldest_request_age {
                oldest_request_age = age;
            }
        }
        
        let average_wait_time = if pending_count > 0 {
            total_wait_time / pending_count as u32
        } else {
            Duration::from_secs(0)
        };
        
        ExecutionStats {
            pending_requests: pending_count,
            average_wait_time,
            oldest_request_age,
        }
    }

    /// Cancel all pending requests
    pub async fn cancel_all_requests(&self) -> Result<usize, PluginError> {
        let mut pending_requests = self.pending_requests.write().await;
        let count = pending_requests.len();
        
        for (_, request) in pending_requests.drain() {
            let _ = request.response_sender.send(Err(PluginError::ExecutionFailed("Request cancelled".to_string())));
        }
        
        Ok(count)
    }

    /// Start cleanup task for expired requests
    pub async fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let pending_requests = Arc::clone(&self.pending_requests);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                let mut requests_to_remove = Vec::new();
                let now = Instant::now();
                
                {
                    let pending_requests = pending_requests.read().await;
                    for (request_id, request) in pending_requests.iter() {
                        let age = now.duration_since(request.start_time);
                        if age > request.timeout {
                            requests_to_remove.push(*request_id);
                        }
                    }
                }
                
                if !requests_to_remove.is_empty() {
                    let mut pending_requests = pending_requests.write().await;
                    for request_id in requests_to_remove {
                        if let Some(request) = pending_requests.remove(&request_id) {
                            let _ = request.response_sender.send(Err(PluginError::ExecutionFailed("Request timeout".to_string())));
                        }
                    }
                }
            }
        })
    }
}

/// Enhanced feature context implementation
pub struct EnhancedFeatureContextImpl {
    execution_id: String,
    plugin_name: String,
    argument_manager: Arc<ArgumentManager>,
    server_context: Arc<dyn ServerContext>,
}

impl EnhancedFeatureContextImpl {
    pub fn new(
        execution_id: String,
        plugin_name: String,
        argument_manager: Arc<ArgumentManager>,
        server_context: Arc<dyn ServerContext>,
    ) -> Self {
        Self {
            execution_id,
            plugin_name,
            argument_manager,
            server_context,
        }
    }
}

#[async_trait::async_trait]
impl FeatureContext for EnhancedFeatureContextImpl {
    async fn get_value(&self, key: &str) -> Option<Value> {
        // Try to get from argument manager
        if let Ok(arg_value) = self.argument_manager.get_argument(
            &self.plugin_name,
            key,
            Some(&self.execution_id),
            super::arguments::ArgumentResolution::UseDefault
        ).await {
            return Some(arg_value.value);
        }
        
        None
    }

    async fn set_value(&self, key: &str, value: Value) -> Result<(), PluginError> {
        // Set in argument manager as a request argument
        self.argument_manager.set_request_argument(
            &self.execution_id,
            key,
            value
        ).await
    }

    async fn get_env(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    async fn log(&self, level: LogLevel, message: &str) {
        let level_str = match level {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        };
        
        println!("[{}] {}: {}", level_str, self.plugin_name, message);
    }

    fn execution_id(&self) -> &str {
        &self.execution_id
    }

    fn plugin_name(&self) -> &str {
        &self.plugin_name
    }
}

/// Execution statistics
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub pending_requests: usize,
    pub average_wait_time: Duration,
    pub oldest_request_age: Duration,
}

/// Enhanced execution request builder
pub struct EnhancedExecutionRequestBuilder {
    provider: Option<String>,
    feature: String,
    action: String,
    arguments: HashMap<String, Value>,
    timeout: Option<Duration>,
}

impl EnhancedExecutionRequestBuilder {
    pub fn new() -> Self {
        Self {
            provider: None,
            feature: String::new(),
            action: String::new(),
            arguments: HashMap::new(),
            timeout: None,
        }
    }

    pub fn provider(mut self, provider: &str) -> Self {
        self.provider = Some(provider.to_string());
        self
    }

    pub fn feature(mut self, feature: &str) -> Self {
        self.feature = feature.to_string();
        self
    }

    pub fn action(mut self, action: &str) -> Self {
        self.action = action.to_string();
        self
    }

    pub fn arguments(mut self, arguments: HashMap<String, Value>) -> Self {
        self.arguments = arguments;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub async fn execute(self, executor: &EnhancedPluginExecutor) -> Result<Value, PluginError> {
        executor.execute_action(
            self.provider.as_deref(),
            &self.feature,
            &self.action,
            self.arguments,
            self.timeout
        ).await
    }
}
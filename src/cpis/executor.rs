//! # Plugin Executor
//!
//! Handles the execution of plugin actions through the event system.
//! No case statements - everything is handled through event callbacks.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde_json::Value;
use uuid::Uuid;
use super::{
    PluginError, FeatureActionEvent, FeatureActionCompleteEvent, 
    EventSystem, FeatureRegistry, ArgumentManager, ServerContext
};

/// Executes plugin actions through the event system
#[derive(Debug)]
pub struct PluginExecutor {
    event_system: Arc<EventSystem>,
    feature_registry: Arc<FeatureRegistry>,
    argument_manager: Arc<ArgumentManager>,
    pending_requests: Arc<RwLock<HashMap<Uuid, PendingRequest>>>,
}

/// Represents a pending action request
#[derive(Debug)]
pub struct PendingRequest {
    pub request_id: Uuid,
    pub feature: String,
    pub action: String,
    pub arguments: HashMap<String, Value>,
    pub start_time: Instant,
    pub timeout: Duration,
    pub response_sender: tokio::sync::oneshot::Sender<Result<Value, PluginError>>,
}

/// Execution context for actions
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub request_id: Uuid,
    pub plugin_name: String,
    pub feature: String,
    pub action: String,
    pub start_time: Instant,
    pub timeout: Duration,
}

/// Execution result with timing information
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub request_id: Uuid,
    pub result: Result<Value, PluginError>,
    pub execution_time: Duration,
    pub plugin_name: String,
}

impl PluginExecutor {
    pub fn new(
        event_system: Arc<EventSystem>,
        feature_registry: Arc<FeatureRegistry>,
        argument_manager: Arc<ArgumentManager>,
    ) -> Self {
        Self {
            event_system,
            feature_registry,
            argument_manager,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize the executor by registering for completion events
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

    /// Execute a feature action
    pub async fn execute_action(
        &self,
        feature: &str,
        action: &str,
        arguments: HashMap<String, Value>,
        timeout: Option<Duration>,
    ) -> Result<Value, PluginError> {
        let request_id = Uuid::new_v4();
        let start_time = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30));

        // Validate that the feature and action exist
        self.feature_registry.validate_action(feature, action, &arguments).await?;

        // Create execution context
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        
        let pending_request = PendingRequest {
            request_id,
            feature: feature.to_string(),
            action: action.to_string(),
            arguments: arguments.clone(),
            start_time,
            timeout,
            response_sender,
        };

        // Store the pending request
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(request_id, pending_request);
        }

        // Emit the feature action event
        let event = FeatureActionEvent {
            feature: feature.to_string(),
            action: action.to_string(),
            arguments,
            request_id,
        };

        let event_key = format!("feature:{}:{}", feature, action);
        self.event_system.emit_event(&event_key, &event).await
            .map_err(|e| PluginError::EventError(e.to_string()))?;

        // Wait for response with timeout
        match tokio::time::timeout(timeout, response_receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                // Response sender was dropped
                self.cleanup_request(request_id).await;
                Err(PluginError::ExecutionFailed("Response channel closed".to_string()))
            },
            Err(_) => {
                // Timeout occurred
                self.cleanup_request(request_id).await;
                Err(PluginError::ExecutionFailed(format!("Action timed out after {:?}", timeout)))
            }
        }
    }

    /// Execute multiple actions in parallel
    pub async fn execute_batch(
        &self,
        batch: Vec<(String, String, HashMap<String, Value>)>, // feature, action, args
        timeout: Option<Duration>,
    ) -> Vec<Result<Value, PluginError>> {
        let futures = batch.into_iter().map(|(feature, action, args)| {
            let executor = self;
            let timeout = timeout;
            async move {
                executor.execute_action(&feature, &action, args, timeout).await
            }
        });

        futures::future::join_all(futures).await
    }

    /// Handle action completion events
    async fn handle_action_complete(
        pending_requests: Arc<RwLock<HashMap<Uuid, PendingRequest>>>,
        event: FeatureActionCompleteEvent,
    ) {
        let mut pending = pending_requests.write().await;
        
        if let Some(pending_request) = pending.remove(&event.request_id) {
            let result = match event.result {
                Ok(value) => Ok(value),
                Err(error_msg) => Err(PluginError::ExecutionFailed(error_msg)),
            };

            // Send the result back to the waiting executor
            let _ = pending_request.response_sender.send(result);
        }
    }

    /// Clean up a request that timed out or failed
    async fn cleanup_request(&self, request_id: Uuid) {
        let mut pending = self.pending_requests.write().await;
        pending.remove(&request_id);
    }

    /// Get statistics about current executions
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        let pending = self.pending_requests.read().await;
        let pending_count = pending.len();
        
        let mut total_wait_time = Duration::ZERO;
        let mut oldest_request = None;
        
        for request in pending.values() {
            let wait_time = request.start_time.elapsed();
            total_wait_time += wait_time;
            
            if oldest_request.is_none() || wait_time > oldest_request.unwrap() {
                oldest_request = Some(wait_time);
            }
        }
        
        let average_wait_time = if pending_count > 0 {
            total_wait_time / pending_count as u32
        } else {
            Duration::ZERO
        };

        ExecutionStats {
            pending_requests: pending_count,
            average_wait_time,
            oldest_request_age: oldest_request.unwrap_or(Duration::ZERO),
        }
    }

    /// Cancel a pending request
    pub async fn cancel_request(&self, request_id: Uuid) -> Result<(), PluginError> {
        let mut pending = self.pending_requests.write().await;
        
        if let Some(request) = pending.remove(&request_id) {
            let _ = request.response_sender.send(Err(PluginError::ExecutionFailed("Request cancelled".to_string())));
            Ok(())
        } else {
            Err(PluginError::ExecutionFailed("Request not found".to_string()))
        }
    }

    /// Get all pending request IDs
    pub async fn get_pending_requests(&self) -> Vec<Uuid> {
        let pending = self.pending_requests.read().await;
        pending.keys().copied().collect()
    }

    /// Cancel all pending requests
    pub async fn cancel_all_requests(&self) -> Result<usize, PluginError> {
        let mut pending = self.pending_requests.write().await;
        let count = pending.len();
        
        for (_, request) in pending.drain() {
            let _ = request.response_sender.send(Err(PluginError::ExecutionFailed("System shutdown".to_string())));
        }
        
        Ok(count)
    }

    /// Execute an action with argument resolution
    pub async fn execute_with_context(
        &self,
        context: Arc<dyn ServerContext>,
        plugin_name: &str,
        feature: &str,
        action: &str,
        user_arguments: HashMap<String, Value>,
        timeout: Option<Duration>,
    ) -> Result<Value, PluginError> {
        let request_id = Uuid::new_v4();
        
        // Get action definition to resolve arguments
        let action_args = self.feature_registry.get_action_arguments(feature, action).await?;
        
        // Resolve all arguments using the argument manager
        let resolved_args = self.argument_manager.resolve_action_arguments(
            plugin_name,
            &action_args,
            &user_arguments,
            Some(&request_id.to_string()),
        ).await?;
        
        // Convert resolved arguments back to simple values
        let final_args: HashMap<String, Value> = resolved_args
            .into_iter()
            .map(|(name, arg_value)| (name, arg_value.value))
            .collect();
        
        // Execute the action
        self.execute_action(feature, action, final_args, timeout).await
    }

    /// Execute an action and emit completion event (for use by plugins)
    pub async fn complete_action(
        &self,
        request_id: Uuid,
        result: Result<Value, String>,
        execution_time_ms: u64,
    ) -> Result<(), PluginError> {
        let completion_event = FeatureActionCompleteEvent {
            request_id,
            result,
            execution_time_ms,
        };

        self.event_system.emit_event("feature:action:complete", &completion_event).await
            .map_err(|e| PluginError::EventError(e.to_string()))?;

        Ok(())
    }

    /// Get execution context for a request
    pub async fn get_execution_context(&self, request_id: Uuid) -> Option<ExecutionContext> {
        let pending = self.pending_requests.read().await;
        pending.get(&request_id).map(|request| {
            ExecutionContext {
                request_id,
                plugin_name: "".to_string(), // Would need to track this separately
                feature: request.feature.clone(),
                action: request.action.clone(),
                start_time: request.start_time,
                timeout: request.timeout,
            }
        })
    }

    /// Set up cleanup task for expired requests
    pub async fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let pending_requests = Arc::clone(&self.pending_requests);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                let mut pending = pending_requests.write().await;
                let now = Instant::now();
                
                // Find expired requests
                let expired_ids: Vec<Uuid> = pending
                    .iter()
                    .filter(|(_, request)| now.duration_since(request.start_time) > request.timeout)
                    .map(|(id, _)| *id)
                    .collect();
                
                // Remove and notify expired requests
                for id in expired_ids {
                    if let Some(request) = pending.remove(&id) {
                        let _ = request.response_sender.send(Err(PluginError::ExecutionFailed("Request expired".to_string())));
                    }
                }
            }
        })
    }
}

/// Statistics about plugin executions
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub pending_requests: usize,
    pub average_wait_time: Duration,
    pub oldest_request_age: Duration,
}

/// Helper for building execution requests
#[derive(Debug)]
pub struct ExecutionRequestBuilder {
    feature: Option<String>,
    action: Option<String>,
    arguments: HashMap<String, Value>,
    timeout: Option<Duration>,
    plugin_name: Option<String>,
}

impl ExecutionRequestBuilder {
    pub fn new() -> Self {
        Self {
            feature: None,
            action: None,
            arguments: HashMap::new(),
            timeout: None,
            plugin_name: None,
        }
    }

    pub fn feature(mut self, feature: impl Into<String>) -> Self {
        self.feature = Some(feature.into());
        self
    }

    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    pub fn argument(mut self, name: impl Into<String>, value: Value) -> Self {
        self.arguments.insert(name.into(), value);
        self
    }

    pub fn arguments(mut self, arguments: HashMap<String, Value>) -> Self {
        self.arguments.extend(arguments);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn plugin_name(mut self, plugin_name: impl Into<String>) -> Self {
        self.plugin_name = Some(plugin_name.into());
        self
    }

    pub async fn execute(self, executor: &PluginExecutor) -> Result<Value, PluginError> {
        let feature = self.feature.ok_or_else(|| PluginError::InvalidArgument("Feature not specified".to_string()))?;
        let action = self.action.ok_or_else(|| PluginError::InvalidArgument("Action not specified".to_string()))?;

        executor.execute_action(&feature, &action, self.arguments, self.timeout).await
    }

    pub async fn execute_with_context(
        self, 
        executor: &PluginExecutor, 
        context: Arc<dyn ServerContext>
    ) -> Result<Value, PluginError> {
        let feature = self.feature.ok_or_else(|| PluginError::InvalidArgument("Feature not specified".to_string()))?;
        let action = self.action.ok_or_else(|| PluginError::InvalidArgument("Action not specified".to_string()))?;
        let plugin_name = self.plugin_name.ok_or_else(|| PluginError::InvalidArgument("Plugin name not specified".to_string()))?;

        executor.execute_with_context(context, &plugin_name, &feature, &action, self.arguments, self.timeout).await
    }
}

impl Default for ExecutionRequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Macro for easy execution request building
#[macro_export]
macro_rules! execute_action {
    (
        executor: $executor:expr,
        feature: $feature:expr,
        action: $action:expr,
        args: { $($arg_name:expr => $arg_value:expr),* $(,)? }
        $(, timeout: $timeout:expr)?
        $(, plugin: $plugin:expr)?
    ) => {{
        let mut builder = $crate::ExecutionRequestBuilder::new()
            .feature($feature)
            .action($action);
        
        $(
            builder = builder.argument($arg_name, $arg_value);
        )*
        
        $(
            builder = builder.timeout($timeout);
        )?
        
        $(
            builder = builder.plugin_name($plugin);
        )?
        
        builder.execute($executor).await
    }};
}

/// Macro for execution with context
#[macro_export]
macro_rules! execute_action_with_context {
    (
        executor: $executor:expr,
        context: $context:expr,
        plugin: $plugin:expr,
        feature: $feature:expr,
        action: $action:expr,
        args: { $($arg_name:expr => $arg_value:expr),* $(,)? }
        $(, timeout: $timeout:expr)?
    ) => {{
        let mut builder = $crate::ExecutionRequestBuilder::new()
            .feature($feature)
            .action($action)
            .plugin_name($plugin);
        
        $(
            builder = builder.argument($arg_name, $arg_value);
        )*
        
        $(
            builder = builder.timeout($timeout);
        )?
        
        builder.execute_with_context($executor, $context).await
    }};
}
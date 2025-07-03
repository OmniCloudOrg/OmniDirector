use crate::cpis::{PluginSystem, PluginExecutor, PluginError};
use rocket::{self, get, post, response::Responder, routes, serde::json::Json};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, sync::Arc, time::Duration};
use serde_json::Value;

// Create the index module
pub mod index;

// Plugin System state stored in application state
pub struct CpiState {
    pub plugin_system: Arc<PluginSystem>,
    pub executor: Arc<PluginExecutor>,
}

// Request format for plugin actions
#[derive(Debug, Deserialize)]
struct PluginActionRequest {
    provider: String, // new field for provider/plugin name
    feature: String,
    action: String,
    #[serde(default)]
    params: HashMap<String, Value>,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    60
}

// Response format for plugin actions
#[derive(Debug, Serialize)]
struct PluginActionResponse {
    success: bool,
    result: Option<Value>,
    error: Option<String>,
    execution_time_ms: u64,
    feature: String,
    action: String,
}

// Custom error handling
#[derive(Debug, Responder)]
enum ApiError {
    #[response(status = 400)]
    BadRequest(String),

    #[response(status = 404)]
    NotFound(String),

    #[response(status = 500)]
    Internal(String),

    #[response(status = 408)]
    Timeout(String),
}

impl From<PluginError> for ApiError {
    fn from(err: PluginError) -> Self {
        match err {
            PluginError::PluginNotFound(name) => {
                ApiError::NotFound(format!("Plugin not found: {}", name))
            }
            PluginError::UnsupportedFeature(name) => {
                ApiError::NotFound(format!("Feature not supported: {}", name))
            }
            PluginError::InvalidArgument(msg) => {
                ApiError::BadRequest(format!("Invalid argument: {}", msg))
            }
            PluginError::ExecutionFailed(msg) if msg.contains("timeout") => {
                ApiError::Timeout(format!("Execution timed out: {}", msg))
            }
            PluginError::ExecutionFailed(msg) => {
                ApiError::Internal(format!("Execution failed: {}", msg))
            }
            _ => ApiError::Internal(err.to_string()),
        }
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

// Route handlers
#[post("/action", format = "json", data = "<action_request>")]
async fn execute_action(
    action_request: Json<PluginActionRequest>,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<PluginActionResponse> {
    let request = action_request.into_inner();
    println!("🎯 Received action request: provider={}, feature={}, action={}", request.provider, request.feature, request.action);

    let start_time = std::time::Instant::now();
    
    // Execute the plugin action (now with provider)
    let timeout = Duration::from_secs(request.timeout_seconds);
    // Use ExecutionRequestBuilder to ensure provider is set
    let result = crate::cpis::executor::ExecutionRequestBuilder::new()
        .plugin_name(&request.provider)
        .feature(&request.feature)
        .action(&request.action)
        .arguments(request.params)
        .timeout(timeout)
        .execute(&cpi_state.executor)
        .await;

    let execution_time_ms = start_time.elapsed().as_millis() as u64;

    let response = match result {
        Ok(value) => {
            println!("✅ Action succeeded in {}ms", execution_time_ms);
            PluginActionResponse {
                success: true,
                result: Some(value),
                error: None,
                execution_time_ms,
                feature: request.feature,
                action: request.action,
            }
        }
        Err(err) => {
            println!("❌ Action failed in {}ms: {}", execution_time_ms, err);
            PluginActionResponse {
                success: false,
                result: None,
                error: Some(err.to_string()),
                execution_time_ms,
                feature: request.feature,
                action: request.action,
            }
        }
    };

    Ok(Json(response))
}

// Get available features (replaces get_providers)

// Get available providers (plugin names)
#[get("/providers")]
async fn get_providers(cpi_state: &rocket::State<CpiState>) -> ApiResult<Vec<String>> {
    let providers = cpi_state.plugin_system.plugin_registry.list_plugins().await;
    Ok(Json(providers))
}

// Get available features (capabilities)
#[get("/features")]
async fn get_features(cpi_state: &rocket::State<CpiState>) -> ApiResult<Vec<String>> {
    let features = cpi_state.plugin_system.get_available_features().await;
    Ok(Json(features))
}

// Get available actions for a feature (replaces get_actions for provider)
#[get("/features/<feature>/actions")]
async fn get_feature_actions(
    feature: String,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<Vec<String>> {
    let actions = cpi_state.plugin_system.get_feature_actions(&feature).await?;
    Ok(Json(actions))
}

// Get all unique actions across all features
#[get("/actions")]
async fn get_all_unique_actions(cpi_state: &rocket::State<CpiState>) -> ApiResult<Vec<String>> {
    let features = cpi_state.plugin_system.get_available_features().await;
    let mut all_actions = Vec::new();

    // Collect actions from all features
    for feature in features {
        match cpi_state.plugin_system.get_feature_actions(&feature).await {
            Ok(feature_actions) => {
                all_actions.extend(feature_actions);
            }
            Err(err) => {
                println!("❌ Error getting actions for feature {}: {}", feature, err);
                // Continue with other features even if one fails
            }
        }
    }

    // Remove duplicates by using a HashSet
    let unique_actions: Vec<String> = all_actions
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    Ok(Json(unique_actions))
}

// Get required parameters for a feature action
#[get("/features/<feature>/actions/<action>/params")]
async fn get_action_params(
    feature: String,
    action: String,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<Vec<String>> {
    let args = cpi_state.plugin_system.get_action_arguments(&feature, &action).await?;
    let param_names: Vec<String> = args.into_iter().map(|arg| arg.name).collect();
    Ok(Json(param_names))
}

// Get detailed parameter definitions for a feature action
#[get("/features/<feature>/actions/<action>/params/detailed")]
async fn get_action_params_detailed(
    feature: String,
    action: String,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<Vec<crate::cpis::features::ArgumentDef>> {
    let args = cpi_state.plugin_system.get_action_arguments(&feature, &action).await?;
    Ok(Json(args))
}

// Execute multiple actions in batch
#[derive(Debug, Deserialize)]
struct BatchRequest {
    actions: Vec<PluginActionRequest>,
    #[serde(default = "default_batch_timeout")]
    timeout_seconds: u64,
}

fn default_batch_timeout() -> u64 {
    120
}

#[derive(Debug, Serialize)]
struct BatchResponse {
    success: bool,
    results: Vec<PluginActionResponse>,
    total_execution_time_ms: u64,
    successful_count: usize,
    failed_count: usize,
}

#[post("/batch", format = "json", data = "<batch_request>")]
async fn execute_batch(
    batch_request: Json<BatchRequest>,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<BatchResponse> {
    let request = batch_request.into_inner();
    println!("📦 Received batch request with {} actions", request.actions.len());

    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(request.timeout_seconds);

    // Convert to the format expected by executor
    let batch_actions: Vec<(String, String, String, HashMap<String, Value>)> = request.actions
        .iter()
        .map(|req| (req.provider.clone(), req.feature.clone(), req.action.clone(), req.params.clone()))
        .collect();

    // Execute batch (now with provider)
    let results = cpi_state.executor.execute_batch(batch_actions, Some(timeout)).await;
    let total_execution_time_ms = start_time.elapsed().as_millis() as u64;

    // Convert results to response format
    let mut response_results = Vec::new();
    let mut successful_count = 0;
    let mut failed_count = 0;

    for (i, result) in results.into_iter().enumerate() {
        let original_request = &request.actions[i];
        
        let response = match result {
            Ok(value) => {
                successful_count += 1;
                PluginActionResponse {
                    success: true,
                    result: Some(value),
                    error: None,
                    execution_time_ms: 0, // Individual timing not available in batch
                    feature: original_request.feature.clone(),
                    action: original_request.action.clone(),
                }
            }
            Err(err) => {
                failed_count += 1;
                PluginActionResponse {
                    success: false,
                    result: None,
                    error: Some(err.to_string()),
                    execution_time_ms: 0,
                    feature: original_request.feature.clone(),
                    action: original_request.action.clone(),
                }
            }
        };
        
        response_results.push(response);
    }

    let batch_response = BatchResponse {
        success: failed_count == 0,
        results: response_results,
        total_execution_time_ms,
        successful_count,
        failed_count,
    };

    println!("📦 Batch completed: {}/{} successful in {}ms", 
             successful_count, successful_count + failed_count, total_execution_time_ms);

    Ok(Json(batch_response))
}

// Get system statistics
#[derive(Debug, Serialize)]
struct SystemStats {
    features_count: usize,
    total_actions: usize,
    event_handlers: usize,
    events_emitted: u64,
    pending_requests: usize,
    global_arguments: usize,
    plugin_arguments: usize,
}

#[get("/stats")]
async fn get_system_stats(cpi_state: &rocket::State<CpiState>) -> ApiResult<SystemStats> {
    let features = cpi_state.plugin_system.get_available_features().await;
    let features_count = features.len();

    // Count total actions
    let mut total_actions = 0;
    for feature in &features {
        if let Ok(actions) = cpi_state.plugin_system.get_feature_actions(feature).await {
            total_actions += actions.len();
        }
    }

    // Get event system stats
    let event_stats = cpi_state.plugin_system.event_system.get_stats().await;
    
    // Get execution stats
    let exec_stats = cpi_state.executor.get_execution_stats().await;
    
    // Get argument stats
    let arg_stats = cpi_state.plugin_system.argument_manager.get_argument_stats().await;

    let stats = SystemStats {
        features_count,
        total_actions,
        event_handlers: event_stats.total_handlers,
        events_emitted: event_stats.events_emitted,
        pending_requests: exec_stats.pending_requests,
        global_arguments: arg_stats.global_arguments,
        plugin_arguments: arg_stats.plugin_arguments,
    };

    Ok(Json(stats))
}

// Health check endpoint
#[get("/health")]
async fn health_check(cpi_state: &rocket::State<CpiState>) -> ApiResult<serde_json::Value> {
    let features = cpi_state.plugin_system.get_available_features().await;
    let exec_stats = cpi_state.executor.get_execution_stats().await;

    let health = serde_json::json!({
        "status": "healthy",
        "features_available": features.len(),
        "pending_requests": exec_stats.pending_requests,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    Ok(Json(health))
}

#[get("/actions/<provider>")]
async fn get_provider_actions_compat(
    provider: String,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<Vec<String>> {
    // Map provider name to feature name and get actions
    get_feature_actions(provider, cpi_state).await
}

#[get("/params/<provider>/<action>")]
async fn get_provider_action_params_compat(
    provider: String,
    action: String,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<Vec<String>> {
    // Map provider/action to feature/action and get params
    get_action_params(provider, action, cpi_state).await
}

pub async fn rocket(
    plugin_system: Arc<PluginSystem>,
    executor: Arc<PluginExecutor>,
) -> rocket::Rocket<rocket::Build> {
    // Load environment variables
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());

    println!("🌐 Server will run at http://{}:{}", host, &port);

    // Log system status
    let features = plugin_system.get_available_features().await;
    println!("📋 Loaded features: {:?}", features);

    // Configure Rocket
    let config = rocket::Config::figment()
        .merge(("address", host))
        .merge(("port", port.parse::<u16>().unwrap()));

    rocket::custom(config)
        .manage(CpiState {
            plugin_system,
            executor,
        })
        .mount(
            "/",
            routes![
                // TODO: Uncomment when index module is ready
                //index::index,
                execute_action,
                get_features,
                get_feature_actions,
                get_action_params,
                get_action_params_detailed,
                get_all_unique_actions,
                execute_batch,
                get_system_stats,
                health_check,
                // Backward compatibility routes
                get_providers,
                get_provider_actions_compat,
                get_provider_action_params_compat,
            ],
        )
}

pub async fn launch_rocket(
    plugin_system: Arc<PluginSystem>,
    executor: Arc<PluginExecutor>,
) {
    // Set up graceful shutdown handler
    let plugin_system_clone = Arc::clone(&plugin_system);
    let executor_clone = Arc::clone(&executor);
    
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\n🛑 Received shutdown signal...");
        if let Err(e) = crate::shutdown_system(plugin_system_clone, executor_clone).await {
            eprintln!("❌ Shutdown error: {}", e);
        }
        std::process::exit(0);
    });

    rocket(plugin_system, executor).await.launch().await.unwrap();
}
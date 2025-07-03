mod api;
mod cpis;
mod logging;

pub mod proposal;

use anyhow::Result;
use std::sync::Arc;
use cpis::{PluginSystem, PluginExecutor, ServerContextBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    println!("🚀 Starting OmniDirector with Event-Driven Plugin System...");
    
    // initialize event system
    println!("🔄 Initializing Event System...");
    let event_system = std::sync::Arc::new(cpis::events::EventSystem::new());

    // Initialize feature registry
    println!("🔍 Initializing Feature Registry...");
    let feature_registry = Arc::new(cpis::features::FeatureRegistry::new());

    // Initialize argument manager
    println!("🛠️ Initializing Argument Manager...");
    let argument_manager = Arc::new(cpis::arguments::ArgumentManager::new());

    // Create server context
    let server_context = ServerContextBuilder::new()
        .with_region_id("default".to_string())
        .with_event_system(event_system.clone())
        .with_feature_registry(feature_registry)
        .with_argument_manager(argument_manager)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create server context: {}", e))?;

    // Initialize the new plugin system
    println!("📦 Initializing Plugin System...");
    let plugin_system = Arc::new(PluginSystem::new(server_context));
    
    // Load features and plugins
    match plugin_system.initialize().await {
        Ok(_) => println!("✅ Plugin system initialized successfully"),
        Err(e) => {
            eprintln!("❌ Failed to initialize plugin system: {}", e);
            return Err(anyhow::anyhow!("Plugin system initialization failed: {}", e));
        }
    }

    // Create plugin executor
    println!("⚡ Setting up Plugin Executor...");
    let executor = Arc::new(PluginExecutor::new(
        plugin_system.event_system.clone(),
        plugin_system.feature_registry.clone(),
        plugin_system.argument_manager.clone(),
    ));

    if let Err(e) = executor.initialize().await {
        eprintln!("❌ Failed to initialize executor: {}", e);
        return Err(anyhow::anyhow!("Executor initialization failed: {}", e));
    }

    // Load global arguments from environment
    println!("🔧 Loading configuration from environment...");
    match plugin_system.argument_manager.load_from_environment("OMNI_").await {
        Ok(count) => println!("📝 Loaded {} arguments from environment", count),
        Err(e) => eprintln!("⚠️  Warning: Failed to load environment arguments: {}", e),
    }

    // Display system status
    display_system_status(&plugin_system, &executor).await?;

    // Start cleanup task for expired requests
    println!("🧹 Starting cleanup task...");
    let _cleanup_handle = executor.start_cleanup_task().await;

    // Launch the API server
    println!("🌐 Starting API server...");
    api::launch_rocket(plugin_system, executor).await;
    
    Ok(())
}

/// Display comprehensive system status
async fn display_system_status(
    plugin_system: &Arc<PluginSystem>,
    executor: &Arc<PluginExecutor>,
) -> Result<()> {
    println!("\n🎯 System Status Report");
    println!("{}", "=".repeat(50));
    
    // Available features
    let features = plugin_system.get_available_features().await;
    println!("📋 Available Features ({}):", features.len());
    for feature in &features {
        println!("  🔹 {}", feature);
        
        // Show actions for each feature
        if let Ok(actions) = plugin_system.get_feature_actions(feature).await {
            println!("    Actions: {}", actions.join(", "));
        }
    }
    
    // Argument statistics
    let arg_stats = plugin_system.argument_manager.get_argument_stats().await;
    println!("\n📊 Argument Statistics:");
    println!("  Global arguments: {}", arg_stats.global_arguments);
    println!("  Plugin arguments: {}", arg_stats.plugin_arguments);
    println!("  Request arguments: {}", arg_stats.request_arguments);
    println!("  Sensitive arguments: {}", arg_stats.sensitive_arguments);
    
    // Event system stats
    let event_stats = plugin_system.event_system.get_stats().await;
    println!("\n📡 Event System Statistics:");
    println!("  Total handlers: {}", event_stats.total_handlers);
    println!("  Events emitted: {}", event_stats.events_emitted);
    
    // Execution stats
    let exec_stats = executor.get_execution_stats().await;
    println!("\n⚡ Execution Statistics:");
    println!("  Pending requests: {}", exec_stats.pending_requests);
    println!("  Average wait time: {:?}", exec_stats.average_wait_time);
    println!("  Oldest request age: {:?}", exec_stats.oldest_request_age);
    
    println!("{}", "=".repeat(50));
    println!("✅ System is ready to serve requests!\n");
    
    Ok(())
}

/// Graceful shutdown handler
pub async fn shutdown_system(
    plugin_system: Arc<PluginSystem>,
    executor: Arc<PluginExecutor>,
) -> Result<()> {
    println!("\n🛑 Initiating graceful shutdown...");
    
    // Cancel pending requests
    let cancelled_count = executor.cancel_all_requests().await
        .unwrap_or_else(|e| {
            eprintln!("⚠️  Warning: Failed to cancel requests: {}", e);
            0
        });
    
    if cancelled_count > 0 {
        println!("🚫 Cancelled {} pending requests", cancelled_count);
    }
    
    // Shutdown plugin system
    if let Err(e) = plugin_system.shutdown().await {
        eprintln!("⚠️  Warning: Plugin system shutdown error: {}", e);
    } else {
        println!("📦 Plugin system shutdown complete");
    }
    
    println!("✅ Graceful shutdown completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    #[tokio::test]
    async fn test_plugin_system_initialization() {
        let server_context = ServerContextBuilder::new()
            .with_region_id("test".to_string())
            .build()
            .expect("Failed to create server context");

        let plugin_system = Arc::new(PluginSystem::new(server_context));
        
        // Should initialize without error
        assert!(plugin_system.initialize().await.is_ok());
        
        // Should have some features available
        let features = plugin_system.get_available_features().await;
        assert!(!features.is_empty());
    }

    #[tokio::test]
    async fn test_executor_initialization() {
        let server_context = ServerContextBuilder::new()
            .with_region_id("test".to_string())
            .build()
            .expect("Failed to create server context");

        let plugin_system = Arc::new(PluginSystem::new(server_context));
        plugin_system.initialize().await.expect("Plugin system init failed");

        let executor = PluginExecutor::new(
            plugin_system.event_system.clone(),
            plugin_system.feature_registry.clone(),
            plugin_system.argument_manager.clone(),
        );

        assert!(executor.initialize().await.is_ok());
    }

    #[tokio::test]
    async fn test_argument_management() {
        let server_context = ServerContextBuilder::new()
            .with_region_id("test".to_string())
            .build()
            .expect("Failed to create server context");

        let plugin_system = Arc::new(PluginSystem::new(server_context));
        plugin_system.initialize().await.expect("Plugin system init failed");

        let args = &plugin_system.argument_manager;
        
        // Set a global argument
        args.set_global_argument("test_arg", Value::String("test_value".to_string()), false)
            .await
            .expect("Failed to set global argument");

        // Retrieve the argument
        let result = args.get_argument("test_plugin", "test_arg", None, cpis::arguments::ArgumentResolution::GlobalOnly).await;
        assert!(result.is_ok());
        
        let arg_value = result.unwrap();
        assert_eq!(arg_value.value, Value::String("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_feature_validation() {
        let server_context = ServerContextBuilder::new()
            .with_region_id("test".to_string())
            .build()
            .expect("Failed to create server context");

        let plugin_system = Arc::new(PluginSystem::new(server_context));
        plugin_system.initialize().await.expect("Plugin system init failed");

        // Test with valid feature
        let features = plugin_system.get_available_features().await;
        if !features.is_empty() {
            let feature = &features[0];
            assert!(plugin_system.feature_registry.is_feature_supported(feature).await);
        }

        // Test with invalid feature
        assert!(!plugin_system.feature_registry.is_feature_supported("NonExistentFeature").await);
    }
}
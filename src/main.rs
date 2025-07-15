mod api;
mod cpis;
mod logging;

pub mod proposal;

use anyhow::Result;
use std::sync::Arc;
use cpis::{PluginSystem, EnhancedPluginExecutor, ServerContextBuilder};

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

    // Initialize the new plugin system, using the event system from the context
    println!("📦 Initializing Plugin System...");
    let plugin_system = Arc::new(PluginSystem::new(server_context.clone() as Arc<dyn cpis::context::ServerContext>));
    
    // Load features and plugins with detailed tracking
    println!("📦 Loading plugins and collecting startup stats...");
    match plugin_system.initialize().await {
        Ok(_) => {
            println!("✅ Plugin system initialized successfully");
            display_plugin_startup_stats(&plugin_system).await?;
        },
        Err(e) => {
            eprintln!("❌ Failed to initialize plugin system: {}", e);
            return Err(anyhow::anyhow!("Plugin system initialization failed: {}", e));
        }
    }

    // Create enhanced plugin executor
    println!("⚡ Setting up Enhanced Plugin Executor...");
    let executor = Arc::new(EnhancedPluginExecutor::new(
        plugin_system.event_system.clone(),
        plugin_system.feature_registry.clone(),
        plugin_system.argument_manager.clone(),
        plugin_system.enhanced_features.clone(),
        server_context.clone(),
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

/// Display plugin startup statistics showing what each plugin added
async fn display_plugin_startup_stats(plugin_system: &Arc<PluginSystem>) -> Result<()> {
    println!("\n📊 Plugin Startup Statistics");
    println!("{}", "=".repeat(60));
    
    // Get loaded plugins
    let plugin_names = plugin_system.get_loaded_plugins().await;
    println!("🔌 Loaded Plugins ({}):", plugin_names.len());
    
    for plugin_name in &plugin_names {
        println!("\n  📦 Plugin: {}", plugin_name);
        
        // Get plugin metadata to show declared features
        if let Some(metadata) = plugin_system.get_plugin_metadata(&plugin_name).await {
            // Show file path
            if let Some(file_path) = &metadata.file_path {
                println!("    📂 File Path: {}", file_path);
            }
            
            println!("    🎯 Declared Features ({}):", metadata.features.len());
            for feature in &metadata.features {
                println!("      • {}", feature);
                
                // Show actions for each declared feature
                if let Ok(actions) = plugin_system.get_feature_actions(feature).await {
                    if !actions.is_empty() {
                        println!("        Actions: {}", actions.join(", "));
                    }
                }
            }
            
            if let Some(description) = &metadata.description {
                println!("    📝 Description: {}", description);
            }
            println!("    📅 Version: {}", metadata.version);
        }
        
        // Show plugin state
        if let Some(state) = plugin_system.get_plugin_state(&plugin_name).await {
            println!("    ⚡ State: {:?}", state);
        }
    }
    
    // Show global event handlers added
    let event_stats = plugin_system.event_system.get_stats().await;
    println!("\n📡 Event System Contributions:");
    println!("  Total Event Handlers: {}", event_stats.total_handlers);
    println!("  Events Emitted: {}", event_stats.events_emitted);
    
    // Show enhanced features (plugin-based features only)
    let enhanced_stats = {
        let enhanced_features = plugin_system.enhanced_features.read().await;
        enhanced_features.get_stats()
    };
    
    if enhanced_stats.total_features > 0 {
        println!("\n🔧 Enhanced Plugin Features:");
        println!("  Total Enhanced Features: {}", enhanced_stats.total_features);
        println!("  Total Enhanced Operations: {}", enhanced_stats.total_operations);
        for (feature, op_count) in enhanced_stats.features {
            println!("    📋 {}: {} operations", feature, op_count);
            
            // Show the available operations for each enhanced feature
            let enhanced_features = plugin_system.enhanced_features.read().await;
            if let Ok(operations) = enhanced_features.get_feature_operations(&feature) {
                for operation in operations {
                    println!("        • {}", operation);
                }
            }
        }
    } else {
        println!("\n🔧 Enhanced Plugin Features:");
        println!("  ✅ No built-in features - using plugins only");
    }
    
    // Show argument contributions
    let arg_stats = plugin_system.argument_manager.get_argument_stats().await;
    println!("\n📝 Argument Contributions:");
    println!("  Global Arguments: {}", arg_stats.global_arguments);
    println!("  Plugin Arguments: {}", arg_stats.plugin_arguments);
    println!("  Sensitive Arguments: {}", arg_stats.sensitive_arguments);
    
    println!("{}", "=".repeat(60));
    println!("✅ Plugin startup analysis complete!\n");
    
    Ok(())
}

/// Display comprehensive system status
async fn display_system_status(
    plugin_system: &Arc<PluginSystem>,
    executor: &Arc<EnhancedPluginExecutor>,
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
    
    // Enhanced feature stats
    let enhanced_stats = {
        let enhanced_features = plugin_system.enhanced_features.read().await;
        enhanced_features.get_stats()
    };
    println!("\n🔧 Enhanced Feature Statistics:");
    println!("  Total features: {}", enhanced_stats.total_features);
    println!("  Total operations: {}", enhanced_stats.total_operations);
    for (feature, op_count) in enhanced_stats.features {
        println!("    {}: {} operations", feature, op_count);
    }
    
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
    executor: Arc<EnhancedPluginExecutor>,
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

        let plugin_system = Arc::new(PluginSystem::new(server_context.clone()));
        plugin_system.initialize().await.expect("Plugin system init failed");

        let executor = EnhancedPluginExecutor::new(
            plugin_system.event_system.clone(),
            plugin_system.feature_registry.clone(),
            plugin_system.argument_manager.clone(),
            plugin_system.enhanced_features.clone(),
            server_context,
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
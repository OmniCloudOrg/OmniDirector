//! # OmniDirector - Unified Provider System
//!
//! Main entry point using the clean unified architecture.

use omni_director::{
    providers::{ProviderRegistry, ProviderLoader, DefaultProviderContext, ProviderContext, FeatureRegistry, EventRegistry},
    routing::Router,
    api::{start_server, ServerConfig},
};
use std::sync::Arc;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("🚀 Starting OmniDirector with unified architecture...");

    // Create provider context
    let context = Arc::new(DefaultProviderContext::new());
    println!("✅ Created provider context");

    // Create provider registry
    let registry = Arc::new(ProviderRegistry::new(Arc::clone(&context) as Arc<dyn ProviderContext>));
    println!("✅ Created provider registry");

    // Create event registry
    let event_registry = Arc::new(EventRegistry::new());
    println!("✅ Created event registry");

    // Create provider loader
    let loader = ProviderLoader::new();
    println!("✅ Created provider loader");

    // Load providers from plugins directory
    println!("📂 Loading providers from ./plugins...");
    let plugin_providers = loader
        .load_from_directory_with_registry("./plugins", Arc::clone(&context) as Arc<dyn ProviderContext>, Some(Arc::clone(&event_registry)))
        .await?;

    for (provider, metadata) in plugin_providers {
        registry.register_provider(provider, metadata).await?;
    }

    // Load feature interfaces from features directory (not as callable providers)
    println!("📂 Loading feature interfaces from ./features...");
    let mut feature_registry = FeatureRegistry::new();
    let feature_interfaces = loader
        .load_features_from_directory("./features")
        .await?;

    for feature_interface in feature_interfaces {
        println!("✅ Loaded feature interface: {}", feature_interface.name);
        feature_registry.register_feature(feature_interface);
    }

    // Validate CPIs against feature interfaces
    println!("🔍 Validating CPI implementations against feature interfaces...");
    validate_cpi_implementations(&registry, &feature_registry, &event_registry).await?;

    // Create router
    let router = Arc::new(Router::new(Arc::clone(&registry)));
    println!("✅ Created router");

    // Get final statistics
    let stats = registry.get_statistics().await;
    println!("📊 Loaded {} providers with {} features and {} operations",
             stats.total_providers, stats.total_features, stats.total_operations);

    // Print loaded providers
    for (name, provider_stats) in &stats.provider_stats {
        println!("  🔌 {}: {} features, {} operations", 
                 name, provider_stats.feature_count, provider_stats.operation_count);
    }

    // Print available routes
    if let Ok(routes) = router.get_available_routes().await {
        println!("\n🔗 Available API routes:");
        for route in routes.iter().take(10) { // Show first 10
            println!("  • {}", route.to_url_path());
        }
        if routes.len() > 10 {
            println!("  ... and {} more routes", routes.len() - 10);
        }
    }

    // Create server configuration
    let config = ServerConfig {
        bind_address: "127.0.0.1:8080".to_string(),
        enable_cors: true,
        enable_logging: true,
        request_timeout_seconds: 30,
    };

    // Start the API server
    println!("🌐 Starting API server...");
    start_server(registry, router, event_registry, config).await?;

    Ok(())
}

/// Validate that CPIs implement the minimum API surface defined by their supported features
async fn validate_cpi_implementations(
    registry: &ProviderRegistry,
    feature_registry: &FeatureRegistry,
    event_registry: &EventRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashSet;
    
    let provider_list = registry.list_providers().await;
    
    for provider_name in provider_list {
        if let Some(metadata) = registry.get_metadata(&provider_name).await {
            // Get the features this CPI claims to support
            let supported_features = metadata.metadata
                .as_ref()
                .and_then(|json| json.get("supports_features"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            
            println!("  🔍 Validating provider '{}' supports features: {:?}", provider_name, supported_features);
            
            for feature_name in &supported_features {
                // Get the feature interface definition
                if let Some(feature_interface) = feature_registry.get_feature(feature_name) {
                    println!("    📋 Checking feature '{}' with {} required operations", 
                             feature_name, feature_interface.operations.len());
                    
                    // Check if all required operations are registered
                    let mut missing_operations = Vec::new();
                    let mut found_operations = Vec::new();
                    
                    for operation in &feature_interface.operations {
                        let event_name = format!("{}.{}", feature_name, operation.name);
                        if event_registry.has_event(&event_name).await {
                            found_operations.push(operation.name.clone());
                        } else {
                            missing_operations.push(operation.name.clone());
                        }
                    }
                    
                    if missing_operations.is_empty() {
                        println!("    ✅ Feature '{}' fully implemented ({} operations)", 
                                 feature_name, found_operations.len());
                    } else {
                        println!("    ❌ Feature '{}' missing operations: {:?}", 
                                 feature_name, missing_operations);
                        return Err(format!(
                            "CPI '{}' claims to support feature '{}' but is missing required operations: {:?}",
                            provider_name, feature_name, missing_operations
                        ).into());
                    }
                } else {
                    println!("    ⚠️  Feature '{}' interface not found", feature_name);
                }
            }
            
            if supported_features.is_empty() {
                println!("    ⚠️  Provider '{}' does not declare any supported features", provider_name);
            }
        }
    }
    
    println!("✅ All CPI implementations validated successfully");
    Ok(())
}
//! # OmniDirector - Unified Provider System
//!
//! Main entry point using the clean unified architecture.

use omni_director::{
    providers::{ProviderRegistry, ProviderLoader, DefaultProviderContext, ProviderContext},
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

    // Create provider loader
    let loader = ProviderLoader::new();
    println!("✅ Created provider loader");

    // Load providers from plugins directory
    println!("📂 Loading providers from ./plugins...");
    let plugin_providers = loader
        .load_from_directory("./plugins", Arc::clone(&context) as Arc<dyn ProviderContext>)
        .await?;

    for (provider, metadata) in plugin_providers {
        registry.register_provider(provider, metadata).await?;
    }

    // Load providers from features directory (as feature adapters)
    println!("📂 Loading features from ./features...");
    let feature_providers = loader
        .load_from_directory("./features", Arc::clone(&context) as Arc<dyn ProviderContext>)
        .await?;

    for (provider, metadata) in feature_providers {
        registry.register_provider(provider, metadata).await?;
    }

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
    start_server(registry, router, config).await?;

    Ok(())
}
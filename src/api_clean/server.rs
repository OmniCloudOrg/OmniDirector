//! # API Server
//!
//! HTTP server with clean routing and middleware.

use super::handlers::*;
use super::middleware::*;
use crate::routing::Router;
use crate::providers::{ProviderRegistry, EventRegistry};
use axum::{
    middleware,
    routing::{get, post},
    Router as AxumRouter,
};
use std::sync::Arc;
use std::time::SystemTime;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// API server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server bind address
    pub bind_address: String,
    /// Enable CORS
    pub enable_cors: bool,
    /// Enable request logging
    pub enable_logging: bool,
    /// Request timeout in seconds
    pub request_timeout_seconds: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:8080".to_string(),
            enable_cors: true,
            enable_logging: true,
            request_timeout_seconds: 30,
        }
    }
}

/// Create the API server with all routes configured
pub fn create_server(
    registry: Arc<ProviderRegistry>,
    router: Arc<Router>,
    event_registry: Arc<EventRegistry>,
    config: ServerConfig,
) -> AxumRouter {
    let state = AppState {
        router,
        registry,
        event_registry,
        start_time: SystemTime::now(),
    };

    let mut app = AxumRouter::new()
        // Health and system endpoints
        .route("/health", get(health))
        .route("/stats", get(get_stats))
        .route("/routes", get(list_routes))
        
        // Provider discovery
        .route("/providers", get(discover_providers))
        .route("/providers/:provider", get(get_provider))
        
        // Feature information
        .route("/providers/:provider/features/:feature", get(get_feature))
        
        // Operation information and execution
        .route(
            "/providers/:provider/features/:feature/operations/:operation",
            get(get_operation).post(execute_operation),
        )
        
        // Unified action execution endpoint
        .route("/exec_action", post(exec_action))
        
        // Add application state
        .with_state(state);

    // Add middleware layers
    if config.enable_logging {
        app = app.layer(TraceLayer::new_for_http());
    }

    if config.enable_cors {
        app = app.layer(CorsLayer::permissive());
    }

    // Add custom middleware
    app = app.layer(middleware::from_fn(request_logging_middleware));
    app = app.layer(middleware::from_fn(error_handling_middleware));

    app
}

/// Start the API server
pub async fn start_server(
    registry: Arc<ProviderRegistry>,
    router: Arc<Router>,
    event_registry: Arc<EventRegistry>,
    config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = create_server(registry, router, event_registry, config.clone());

    println!("🚀 Starting OmniDirector API server on {}", config.bind_address);
    println!("📚 API Documentation:");
    print_api_routes();

    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

/// Print available API routes for documentation
fn print_api_routes() {
    println!("  GET  /health                     - Health check");
    println!("  GET  /stats                      - Registry statistics");
    println!("  GET  /routes                     - List all available routes");
    println!("  GET  /providers                  - Discover all providers");
    println!("  GET  /providers/{{provider}}       - Get provider information");
    println!("  GET  /providers/{{provider}}/features/{{feature}} - Get feature information");
    println!("  GET  /providers/{{provider}}/features/{{feature}}/operations/{{operation}} - Get operation information");
    println!("  POST /providers/{{provider}}/features/{{feature}}/operations/{{operation}} - Execute operation");
    println!();
    println!("🔗 Example URLs:");
    println!("  http://127.0.0.1:8080/health");
    println!("  http://127.0.0.1:8080/providers");
    println!("  http://127.0.0.1:8080/providers/worker-management/features/worker_management/operations/list-workers");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ProviderContext, DefaultProviderContext};
    use axum::http::StatusCode;
    use axum_test::TestServer;

    #[tokio::test]
    async fn test_server_routes() {
        let context = Arc::new(DefaultProviderContext::new());
        let registry = Arc::new(ProviderRegistry::new(context));
        let router = Arc::new(Router::new(Arc::clone(&registry)));
        let config = ServerConfig::default();
        
        let app = create_server(registry, router, config);
        let server = TestServer::new(app).unwrap();

        // Test health endpoint
        let response = server.get("/health").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        // Test discovery endpoint
        let response = server.get("/providers").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        // Test routes endpoint
        let response = server.get("/routes").await;
        assert_eq!(response.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_provider_not_found() {
        let context = Arc::new(DefaultProviderContext::new());
        let registry = Arc::new(ProviderRegistry::new(context));
        let router = Arc::new(Router::new(Arc::clone(&registry)));
        let config = ServerConfig::default();
        
        let app = create_server(registry, router, config);
        let server = TestServer::new(app).unwrap();

        // Test non-existent provider
        let response = server.get("/providers/nonexistent").await;
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }
}
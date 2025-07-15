//! # OmniDirector - Unified Provider System
//!
//! A clean, unified architecture for managing cloud providers and their features.
//! 
//! ## Architecture Overview
//! 
//! The OmniDirector system is built around a unified provider architecture:
//! 
//! - **Providers**: External services (VirtualBox, AWS, Docker, etc.)
//! - **Features**: Capabilities within providers (vm-management, storage, etc.)
//! - **Operations**: Actions within features (start, stop, create, etc.)
//! 
//! ## Usage
//! 
//! ```rust
//! use omni_director::{
//!     providers::{ProviderRegistry, ProviderLoader, DefaultProviderContext},
//!     routing::Router,
//!     api_clean::{start_server, ServerConfig},
//! };
//! 
//! // Create provider context
//! let context = Arc::new(DefaultProviderContext::new());
//! 
//! // Create provider registry
//! let registry = Arc::new(ProviderRegistry::new(context));
//! 
//! // Load providers
//! let loader = ProviderLoader::new();
//! let providers = loader.load_from_directory("./plugins", context).await?;
//! 
//! // Register providers
//! for (provider, metadata) in providers {
//!     registry.register_provider(provider, metadata).await?;
//! }
//! 
//! // Create router
//! let router = Arc::new(Router::new(registry));
//! 
//! // Start server
//! let config = ServerConfig::default();
//! start_server(registry, router, config).await?;
//! ```
//! 
//! ## API Endpoints
//! 
//! - `GET /health` - Health check
//! - `GET /providers` - List all providers
//! - `GET /providers/{provider}` - Get provider details
//! - `GET /providers/{provider}/features/{feature}` - Get feature details  
//! - `POST /providers/{provider}/features/{feature}/operations/{operation}` - Execute operation

// Core modules
pub mod core;
pub mod providers;
pub mod routing;

// API module (renamed from api_new)
pub mod api {
    //! Clean API layer with RESTful endpoints
    pub use crate::api_clean::*;
}

// Internal API implementation
mod api_clean {
    pub mod handlers;
    pub mod server;
    pub mod middleware;
    pub mod responses;
    
    pub use handlers::*;
    pub use server::*;
    pub use middleware::*;
    pub use responses::*;
}

// Re-exports for convenience
pub use core::*;
pub use providers::*;
pub use routing::*;

// Common types and utilities
pub mod prelude {
    //! Common imports for OmniDirector usage
    pub use crate::providers::{
        Provider, ProviderContext, ProviderRegistry, ProviderLoader,
        DefaultProviderContext, ProviderMetadata, ProviderError, ProviderResult,
    };
    pub use crate::routing::{Router, Route};
    pub use crate::api::{ServerConfig, start_server};
    pub use std::sync::Arc;
    pub use std::collections::HashMap;
    pub use serde_json::Value;
}
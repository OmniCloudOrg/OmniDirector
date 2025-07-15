//! # API Middleware
//!
//! Middleware components for request processing.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::time::Instant;

/// Request logging middleware
pub async fn request_logging_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    
    println!("📥 {} {}", method, uri);
    
    let response = next.run(request).await;
    
    let duration = start.elapsed();
    let status = response.status();
    
    println!("📤 {} {} {} ({:.2}ms)", method, uri, status, duration.as_secs_f64() * 1000.0);
    
    Ok(response)
}

/// Error handling middleware
pub async fn error_handling_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let response = next.run(request).await;
    
    // Log errors if status code indicates an error
    if response.status().is_client_error() || response.status().is_server_error() {
        println!("❌ Error response: {}", response.status());
    }
    
    Ok(response)
}

/// CORS middleware (if not using tower-http)
pub async fn cors_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(request).await;
    
    let headers = response.headers_mut();
    headers.insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    headers.insert("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap());
    headers.insert("Access-Control-Allow-Headers", "Content-Type, Authorization".parse().unwrap());
    
    Ok(response)
}

/// Authentication middleware (placeholder for future implementation)
pub async fn auth_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // TODO: Implement authentication logic
    // For now, just pass through all requests
    Ok(next.run(request).await)
}

/// Rate limiting middleware (placeholder for future implementation)
pub async fn rate_limit_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // TODO: Implement rate limiting logic
    // For now, just pass through all requests
    Ok(next.run(request).await)
}
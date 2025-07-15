//! # Dynamic API Endpoints
//!
//! This module provides REST API endpoints for the new enum-based feature system.
//! Users can request resources generically and get provider-specific responses.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;


/// API request for creating a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResourceRequest {
    /// Resource type (vm, worker, storage, etc.)
    pub resource_type: String,
    /// Feature name (worker_management, vm_management, etc.)
    pub feature: String,
    /// Operation (StartWorker, CreateVM, etc.)
    pub operation: String,
    /// Parameters for the operation
    pub parameters: Option<HashMap<String, Value>>,
}

/// API response for successful operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub request_id: String,
}
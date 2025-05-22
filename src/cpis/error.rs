use std::error::Error;
use thiserror::Error;
use std::fmt;

#[derive(Error, Debug)]
/// Enum representing various errors that can occur in the CPI (Cloud Provider Interface) context.
pub enum CpiError {
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),
    
    #[error("Action not found: {0}")]
    ActionNotFound(String),
    
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),
    
    #[error("Invalid parameter type for {0}, expected {1}")]
    InvalidParameterType(String, String),
    
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Failed to parse command output: {0}")]
    ParseError(String),
    
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    #[error("Invalid CPI format: {0}")]
    InvalidCpiFormat(String),
    
    #[error("No CPI providers were successfully loaded")]
    NoProvidersLoaded,
    
    #[error("Command timed out: {0}")]
    Timeout(String),
    
    #[error("File error: {0}")]
    FileError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    SerdeError(#[from] serde_json::Error),
    
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("CPI name collision: {0}")]
    CpiNameCollision(String),

    #[error("Library loading error: {0}")]
    LibraryLoadingError(#[from] libloading::Error),

    #[error("Failed to initialize the CPI system: {0}, {1}")]
    InitializationFailedError(String, #[source] anyhow::Error),

    #[error("Malformed CPI {0}")]
    MalformedCpiError(String),
}

/// Helper function to get a user-friendly error message
/// 
/// This function takes a `CpiError` and returns a string with a user-friendly
/// error message. It provides more context about the error and suggests
/// possible actions to resolve it.
pub fn user_friendly_error(err: &CpiError) -> String {
    match err {
        CpiError::ProviderNotFound(name) => 
            format!("The CPI provider '{}' could not be found. Please check the provider name and ensure it's correctly installed.", name),
        CpiError::ActionNotFound(name) => 
            format!("The action '{}' could not be found. Please check the action name and provider documentation.", name),
        CpiError::MissingParameter(name) => 
            format!("The required parameter '{}' was not provided. Please include this parameter and try again.", name),
        CpiError::InvalidParameterType(name, expected) => 
            format!("The parameter '{}' has an invalid type. Expected {}.", name, expected),
        CpiError::ExecutionFailed(reason) => 
            format!("The command failed to execute: {}", reason),
        CpiError::ParseError(reason) => 
            format!("Failed to parse the command output: {}", reason),
        CpiError::InvalidPath(reason) => 
            format!("Invalid path: {}", reason),
        CpiError::InvalidCpiFormat(reason) => 
            format!("The CPI file has an invalid format: {}", reason),
        CpiError::NoProvidersLoaded => 
            "No CPI providers were successfully loaded. Please check the CPIs directory and ensure valid provider files exist.".to_string(),
        CpiError::IoError(e) => 
            format!("I/O error occurred: {}", e),
        CpiError::SerdeError(e) => 
            format!("JSON parsing error: {}", e),
        CpiError::RegexError(e) => 
            format!("Regular expression error: {}", e),
        CpiError::Timeout(cmd) => 
            format!("The command '{}' timed out. Please check if it's hanging or taking too long.", cmd),
        CpiError::FileError(reason) => 
            format!("File error: {}", reason),
        CpiError::CpiNameCollision(name) =>
            format!("CPI name collision detected: '{}'. Please ensure that no two CPIs have the same name.", name),
        CpiError::LibraryLoadingError(e) =>
            format!("Failed to load the library: {}", e),
        CpiError::InitializationFailedError(name, e) =>
            format!("Initialization failed for CPI {} OS error: {}", name, e),
        CpiError::MalformedCpiError(name) =>
            format!("The CPI '{0}' is malformed. Please check the CPI format and ensure it adheres to the expected structure.", name),
    }
}

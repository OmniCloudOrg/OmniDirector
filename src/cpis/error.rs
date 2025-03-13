use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub enum CpiError {
    ProviderNotFound(String),
    ActionNotFound(String),
    MissingParameter(String),
    InvalidParameterType(String, String),
    ExecutionFailed(String),
    ParseError(String),
    InvalidPath(String),
    InvalidCpiFormat(String),
    NoProvidersLoaded,
    IoError(std::io::Error),
    SerdeError(serde_json::Error),
}

impl fmt::Display for CpiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpiError::ProviderNotFound(name) => write!(f, "Provider not found: {}", name),
            CpiError::ActionNotFound(name) => write!(f, "Action not found: {}", name),
            CpiError::MissingParameter(name) => write!(f, "Missing required parameter: {}", name),
            CpiError::InvalidParameterType(name, expected) => 
                write!(f, "Invalid parameter type for {}, expected {}", name, expected),
            CpiError::ExecutionFailed(reason) => write!(f, "Command execution failed: {}", reason),
            CpiError::ParseError(reason) => write!(f, "Failed to parse command output: {}", reason),
            CpiError::InvalidPath(reason) => write!(f, "Invalid path: {}", reason),
            CpiError::InvalidCpiFormat(reason) => write!(f, "Invalid CPI format: {}", reason),
            CpiError::NoProvidersLoaded => write!(f, "No CPI providers were successfully loaded"),
            CpiError::IoError(e) => write!(f, "IO error: {}", e),
            CpiError::SerdeError(e) => write!(f, "JSON error: {}", e),
        }
    }
}

impl Error for CpiError {}

impl From<std::io::Error> for CpiError {
    fn from(err: std::io::Error) -> Self {
        CpiError::IoError(err)
    }
}

impl From<serde_json::Error> for CpiError {
    fn from(err: serde_json::Error) -> Self {
        CpiError::SerdeError(err)
    }
}
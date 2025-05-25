// error.rs - Enhanced error handling
use std::error::Error;
use std::fmt;

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
    SerdeError(Box<dyn std::error::Error + Send + Sync>),
    RegexError(regex::Error),
    Timeout(String),
    FileError(String),
}

impl fmt::Display for CpiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpiError::ProviderNotFound(name) => write!(f, "Provider not found: {}", name),
            CpiError::ActionNotFound(name) => write!(f, "Action not found: {}", name),
            CpiError::MissingParameter(name) => write!(f, "Missing required parameter: {}", name),
            CpiError::InvalidParameterType(name, expected) => write!(
                        f,
                        "Invalid parameter type for {}, expected {}",
                        name, expected
                    ),
            CpiError::ExecutionFailed(reason) => write!(f, "Command execution failed: {}", reason),
            CpiError::ParseError(reason) => write!(f, "Failed to parse command output: {}", reason),
            CpiError::InvalidPath(reason) => write!(f, "Invalid path: {}", reason),
            CpiError::InvalidCpiFormat(reason) => write!(f, "Invalid CPI format: {}", reason),
            CpiError::NoProvidersLoaded => write!(f, "No CPI providers were successfully loaded"),
            CpiError::IoError(e) => write!(f, "IO error: {}", e),
            CpiError::SerdeError(e) => write!(f, "JSON error: {}", e),
            CpiError::RegexError(e) => write!(f, "Regex error: {}", e),
            CpiError::Timeout(cmd) => write!(f, "Command timed out: {}", cmd),
            CpiError::FileError(_) => write!(f, "File error"),
        }
    }
}

impl Error for CpiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CpiError::IoError(e) => Some(e),
            CpiError::SerdeError(e) => Some(e.as_ref()),
            CpiError::RegexError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CpiError {
    fn from(err: std::io::Error) -> Self {
        CpiError::IoError(err)
    }
}

impl From<serde_json::Error> for CpiError {
    fn from(err: serde_json::Error) -> Self {
        CpiError::SerdeError(Box::new(err))
    }
}

impl From<regex::Error> for CpiError {
    fn from(err: regex::Error) -> Self {
        CpiError::RegexError(err)
    }
}

// Helper function to get a user-friendly error message
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
        CpiError::FileError(_) => 
                format!("File error"),
    }
}

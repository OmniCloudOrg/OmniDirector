// mod.rs - Complete improved initialization with better logging

pub mod parser;
pub mod executor;
pub mod provider;
pub mod error;
pub mod validator;
pub mod logger;

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs::{self, File};
use std::sync::Arc;
use serde_json::Value;
use self::error::CpiError;
use self::provider::Provider;
use self::validator::validate_cpi_format;
use log::{info, warn, error, debug, trace};
use rayon::prelude::*;

pub fn initialize() -> Result<CpiSystem, error::CpiError> {
    info!("Initializing CPI system");
    let start = std::time::Instant::now();
    
    // Configure logging based on environment
    logger::configure_from_env();
    
    let mut system = CpiSystem::new();
    match system.load_all_providers() {
        Ok(count) => {
            let duration = start.elapsed();
            info!("CPI system initialized with {} providers in {:?}", count, duration);
            Ok(system)
        },
        Err(e) => {
            error!("Failed to initialize CPI system: {}", e);
            Err(e)
        }
    }
}

// Public API for the CPI system
pub struct CpiSystem {
    providers: HashMap<String, Arc<Provider>>,
}

impl CpiSystem {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }
    
    // Load all providers from the ./CPIs directory with enhanced error reporting
    pub fn load_all_providers(&mut self) -> Result<usize, CpiError> {
        // Try multiple possible locations for the CPIs directory
        let possible_paths = [
            Path::new("./CPIs"),
            Path::new("CPIs"),
            Path::new("../CPIs"),
            Path::new("./cpi_system/CPIs"),
            // Add current executable directory + CPIs
            &std::env::current_exe().ok().map(|mut p| {
                p.pop();
                p.push("CPIs");
                p
            }).unwrap_or_else(|| Path::new("./exe_dir/CPIs").to_path_buf()),
        ];
        
        let mut cpi_dir = None;
        for path in &possible_paths {
            info!("Checking for CPIs directory at: {:?}", path);
            if path.exists() && path.is_dir() {
                cpi_dir = Some(path);
                info!("Found CPIs directory at: {:?}", path);
                break;
            }
        }
        
        let cpi_dir = match cpi_dir {
            Some(dir) => dir,
            None => {
                let err_msg = format!(
                    "Could not find CPIs directory. Checked paths: {:?}", 
                    possible_paths
                );
                error!("{}", err_msg);
                return Err(CpiError::InvalidPath(err_msg));
            }
        };
        
        // List all files in the directory for debugging
        info!("Examining contents of CPIs directory:");
        let dir_contents = match fs::read_dir(cpi_dir) {
            Ok(entries) => entries,
            Err(e) => {
                error!("Failed to read CPIs directory: {}", e);
                return Err(CpiError::IoError(e));
            }
        };
        
        for entry in dir_contents {
            if let Ok(entry) = entry {
                info!("Found file: {:?}", entry.path());
            }
        }
        
        // Now actually read the directory for processing
        let entries = match fs::read_dir(cpi_dir) {
            Ok(entries) => entries,
            Err(e) => {
                error!("Failed to read CPIs directory: {}", e);
                return Err(CpiError::IoError(e));
            }
        };
        
        let mut all_files = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Error reading directory entry: {}", e);
                    continue;
                }
            };
            
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "json" {
                        all_files.push(path);
                    } else {
                        debug!("Skipping non-JSON file: {:?}", path);
                    }
                } else {
                    debug!("Skipping file without extension: {:?}", path);
                }
            }
        }
        
        if all_files.is_empty() {
            let err_msg = format!("No JSON files found in CPIs directory: {:?}", cpi_dir);
            error!("{}", err_msg);
            return Err(CpiError::NoProvidersLoaded);
        }
        
        info!("Found {} potential CPI definition files", all_files.len());
        
        // Process each file
        let mut loaded_count = 0;
        let mut validation_errors = Vec::new();
        let mut loading_errors = Vec::new();
        
        let total_files = all_files.len();
        for path in all_files {
            info!("Processing CPI file: {:?}", path);
            
            // Validate the JSON before loading
            if let Err(e) = self.validate_provider_file(&path) {
                let err_msg = format!("Failed to validate CPI file {:?}: {}", path, e);
                warn!("{}", err_msg);
                validation_errors.push((path.clone(), err_msg));
                continue;
            }
            
            // Load the provider
            match self.register_provider(path.clone()) {
                Ok(_) => {
                    info!("Successfully loaded CPI from {:?}", path);
                    loaded_count += 1;
                },
                Err(e) => {
                    let err_msg = format!("Failed to load CPI file {:?}: {}", path, e);
                    warn!("{}", err_msg);
                    loading_errors.push((path.clone(), err_msg));
                }
            }
        }
        
        // Report detailed errors if no providers were loaded
        if loaded_count == 0 {
            error!("No CPI providers were successfully loaded.");
            
            if !validation_errors.is_empty() {
                error!("Validation errors:");
                for (path, err) in &validation_errors {
                    error!("  {:?}: {}", path, err);
                }
            }
            
            if !loading_errors.is_empty() {
                error!("Loading errors:");
                for (path, err) in &loading_errors {
                    error!("  {:?}: {}", path, err);
                }
            }
            
            return Err(CpiError::NoProvidersLoaded);
        }
        // Report successful loading with clear formatting for visibility
        info!("============================================");
        info!("✅ Successfully loaded {}/{} CPI providers", loaded_count, total_files);
        info!("============================================");

        
        // Show warnings about failed providers
        if !validation_errors.is_empty() || !loading_errors.is_empty() {
            let total_errors = validation_errors.len() + loading_errors.len();
            warn!("{} CPI providers failed to load", total_errors);
        }
        
        Ok(loaded_count)
    }

    // Enhanced validation with better error reporting
    fn validate_provider_file(&self, path: &Path) -> Result<(), CpiError> {
        debug!("Validating CPI file: {:?}", path);
        
        // Check file exists and is readable
        if !path.exists() {
            let err_msg = format!("File does not exist: {:?}", path);
            error!("{}", err_msg);
            return Err(CpiError::InvalidPath(err_msg));
        }
        
        // Try to read the file
        let file_content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                let err_msg = format!("Failed to read file {:?}: {}", path, e);
                error!("{}", err_msg);
                return Err(CpiError::IoError(e));
            }
        };
        
        // Check if file is empty
        if file_content.trim().is_empty() {
            let err_msg = format!("File is empty: {:?}", path);
            error!("{}", err_msg);
            return Err(CpiError::InvalidCpiFormat(err_msg));
        }
        
        // Try to parse the JSON
        let json: Value = match serde_json::from_str(&file_content) {
            Ok(json) => json,
            Err(e) => {
                let err_msg = format!("Failed to parse JSON in file {:?}: {}", path, e);
                error!("{}", err_msg);
                
                // Provide more details about parse errors
                let line_col = find_json_error_location(&file_content, &e);
                if let Some((line, col)) = line_col {
                    error!("JSON error at line {}, column {}", line, col);
                    if let Some(problematic_line) = file_content.lines().nth(line - 1) {
                        error!("Problematic line: {}", problematic_line);
                        error!("{}^", " ".repeat(col - 1));
                    }
                }
                
                return Err(CpiError::SerdeError(e));
            }
        };
        
        // Validate the JSON structure
        match validate_cpi_format(&json, Some(path)) {
            Ok(_) => {
                debug!("CPI file {:?} validated successfully", path);
                Ok(())
            },
            Err(e) => {
                error!("CPI file {:?} validation failed: {}", path, e);
                Err(e)
            }
        }
    }
    
    // Enhanced provider registration with better error reporting
    pub fn register_provider(&mut self, path: PathBuf) -> Result<(), CpiError> {
        info!("Registering provider from: {:?}", path);
        
        let provider = match provider::load_provider(path.clone()) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to load provider from {:?}: {}", path, e);
                return Err(e);
            }
        };
        
        info!("Successfully loaded provider '{}' ({} actions) from {:?}", 
             provider.name, provider.actions.len(), path);
        
        self.providers.insert(provider.name.clone(), Arc::new(provider));
        Ok(())
    }
    
    // Get available providers
    pub fn get_providers(&self) -> Vec<String> {
        let providers = self.providers.keys().cloned().collect();
        debug!("Available providers: {:?}", providers);
        providers
    }

    // Get available actions for a provider
    pub fn get_provider_actions(&self, provider_name: &str) -> Result<Vec<String>, CpiError> {
        let provider = self.get_provider(provider_name)?;
        let actions = provider.actions.keys().cloned().collect();
        debug!("Actions for provider '{}': {:?}", provider_name, actions);
        Ok(actions)
    }

    // Get the required parameters for an action
    pub fn get_action_params(&self, provider_name: &str, action_name: &str) -> Result<Vec<String>, CpiError> {
        let provider = self.get_provider(provider_name)?;
        let action = provider.get_action(action_name)?;
        
        let params = if let Some(params) = &action.params {
            params.clone()
        } else {
            Vec::new()
        };
        
        debug!("Parameters for action '{}' in provider '{}': {:?}", 
              action_name, provider_name, params);
        
        Ok(params)
    }

    // Execute a CPI action
    pub fn execute(&self, provider_name: &str, action_name: &str, params: HashMap<String, Value>) -> Result<Value, CpiError> {
        let provider = self.get_provider(provider_name)?;
        info!("Executing action '{}' from provider '{}'", action_name, provider_name);
        let start = std::time::Instant::now();
        
        let result = executor::execute_action(provider, action_name, params);
        
        let duration = start.elapsed();
        if let Ok(_) = &result {
            info!("Action '{}' from provider '{}' completed successfully in {:?}", 
                 action_name, provider_name, duration);
        } else {
            error!("Action '{}' from provider '{}' failed after {:?}", 
                  action_name, provider_name, duration);
        }
        
        result
    }

    // Helper method to get a provider
    fn get_provider(&self, provider_name: &str) -> Result<&Provider, CpiError> {
        self.providers.get(provider_name)
            .map(|arc| arc.as_ref())
            .ok_or_else(|| {
                let available = self.providers.keys().cloned().collect::<Vec<_>>().join(", ");
                let err_msg = format!("Provider '{}' not found. Available providers: {}", 
                                     provider_name, available);
                error!("{}", err_msg);
                CpiError::ProviderNotFound(provider_name.to_string())
            })
    }
}

// Helper function to find line and column of JSON parse errors
fn find_json_error_location(content: &str, error: &serde_json::Error) -> Option<(usize, usize)> {
    let column = error.column();
    let line = error.line();
    if line > 0 && column > 0 {
        Some((line, column))
    } else {
        None
    }
}
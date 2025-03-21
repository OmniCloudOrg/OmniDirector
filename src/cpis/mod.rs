// mod.rs - Optimized initialization with better logging

pub mod error;
pub mod executor;
pub mod loader;
pub mod logger;
pub mod parser;
pub mod provider;
pub mod validator;

use self::error::CpiError;
use self::provider::Provider;
use self::validator::validate_cpi_format;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use rayon::prelude::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(debug_assertions)]
fn time<T, A: ToString, F: FnOnce() -> T>(name: A, f: F) -> T {
    let time = std::time::Instant::now();
    let out = f();
    let action = name.to_string();
    debug!("{} Took {:?}", action, time.elapsed());
    out
}
#[cfg(not(debug_assertions))]
fn time<T, A: ToString, F: FnOnce() -> T>(_: A, f: F) -> T {
    f()
}

pub fn initialize() -> Result<CpiSystem, error::CpiError> {
    info!("Initializing CPI system");
    let start = std::time::Instant::now();

    // Configure logging based on environment
    let _ = logger::configure_from_env();

    let system = CpiSystem::new();
    match system.load_all_providers() {
        Ok(count) => {
            let duration = start.elapsed();
            info!(
                "CPI system initialized with {} providers in {:?}",
                count, duration
            );
            Ok(system)
        }
        Err(e) => {
            error!("Failed to initialize CPI system: {}", e);
            Err(e)
        }
    }
}

// Public API for the CPI system
pub struct CpiSystem {
    providers: DashMap<String, Provider>,
}

impl CpiSystem {
    pub fn new() -> Self {
        Self {
            providers: DashMap::new(),
        }
    }

    // Load all providers from the ./CPIs directory with enhanced error reporting
    pub fn load_all_providers(&self) -> Result<usize, CpiError> {
        info!("Loading all CPI providers from ./CPIs directory");

        // Load CPIs using the loader module
        let cpis = Arc::new(loader::load_cpis());
        let total_cpis = cpis.len();
        
        // Use a concurrent HashSet for tracking valid CPIs
        let valid_cpis = DashMap::new();
        
        // Process all CPIs in parallel
        cpis.iter().par_bridge().for_each(|entry| {
            let cpi_key = entry.key();
            let cpi_value = entry.value();
            
            debug!("Validating provider file: {:?}", cpi_key);

            // Avoid multiple JSON parse operations by parsing once
            match serde_json::from_str::<Value>(cpi_value) {
                Ok(json_value) => {
                    if let Ok(provider) = serde_json::from_str::<Provider>(cpi_value) {
                        // Validate in parallel
                        if self.validate_provider_file(
                            PathBuf::from(cpi_key),
                            &json_value,
                        ).is_ok() {
                            // Register directly if valid
                            if let Ok(()) = self.register_provider_direct(cpi_key.to_string(), provider) {
                                valid_cpis.insert(cpi_key.to_string(), true);
                                debug!("Successfully registered provider '{}'", cpi_key);
                            } else {
                                error!("Failed to register provider '{}'", cpi_key);
                            }
                        }
                    } else {
                        error!("Failed to parse provider from JSON for '{}'", cpi_key);
                    }
                }
                Err(e) => {
                    error!("Failed to parse JSON for CPI '{}': {}", cpi_key, e);
                }
            }
        });

        let loaded_count = self.providers.len();
        if loaded_count == 0 {
            return Err(CpiError::NoProvidersLoaded);
        }

        // Collect failed CPIs
        let failed_cpis: Vec<String> = cpis
            .iter()
            .filter_map(|entry| {
                let key = entry.key().to_string();
                if !valid_cpis.contains_key(&key) {
                    Some(key)
                } else {
                    None
                }
            })
            .collect();

        // Report successful loading with clear formatting for visibility
        info!("============================================");
        info!(
            "✅ Successfully loaded {}/{} CPI providers",
            loaded_count, total_cpis
        );
        
        // Only display failed CPIs if there are any
        if !failed_cpis.is_empty() {
            warn!("===============================================");
            warn!("❌ Failed to load {} providers:", failed_cpis.len());
            for failed in &failed_cpis {
                warn!("   - {}", failed);
            }
            warn!("===============================================");
        }
        
        info!("Successfully validated: {}/{} providers", valid_cpis.len(), total_cpis);
        info!("============================================");

        Ok(loaded_count)
    }

    // Optimized validation with direct JSON value
    fn validate_provider_file(&self, path: PathBuf, json: &Value) -> Result<(), CpiError> {
        debug!("Validating CPI file: {}", path.display());

        // Validate the JSON structure
        match path.to_str() {
            Some(path_str) => validate_cpi_format(path_str, json),
            None => Err(CpiError::InvalidPath("Invalid path".to_string())),
        }
    }

    // Direct provider registration - optimized version
    fn register_provider_direct(
        &self,
        provider_name: String,
        provider: Provider,
    ) -> Result<(), CpiError> {
        debug!("Registering provider: {:?}", provider.name);
        self.providers.insert(provider.name.clone(), provider);
        Ok(())
    }

    // Enhanced provider registration with better error reporting
    pub fn register_provider(
        &self,
        provider_name: String,
        provider_content: String,
        should_test: bool,
    ) -> Result<(), CpiError> {
        // Map string to provider struct using serde
        let provider: Provider = serde_json::from_str(&provider_content).map_err(|e| {
            error!("Failed to parse JSON for provider {} Error: {}", provider_name, e);
            CpiError::SerdeError(e)
        })?;
        debug!("Registering provider: {:?}", provider.name);

        if should_test {
            info!("Running test command on provider '{}'", provider.name);
            let test_result = executor::execute_action(&provider, "test_install", HashMap::new());

            match test_result {
                Ok(_) => {
                    debug!("Test command succeeded for provider '{}'", provider.name);
                }
                Err(e) => {
                    error!("Test command failed for provider '{}': {}", provider.name, e);
                    return Err(e);
                }
            }
        }

        self.providers.insert(provider.name.clone(), provider);
        Ok(())
    }

    // Get available providers
    pub fn get_providers(&self) -> Vec<String> {
        self.providers
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    // Get available actions for a provider
    pub fn get_provider_actions(&self, provider_name: &str) -> Result<Vec<String>, CpiError> {
        let provider = self.get_provider(provider_name)?;
        Ok(provider.actions.keys().cloned().collect())
    }

    // Get the required parameters for an action
    pub fn get_action_params(
        &self,
        provider_name: &str,
        action_name: &str,
    ) -> Result<Vec<String>, CpiError> {
        let provider = self.get_provider(provider_name)?;
        let action = provider.get_action(action_name)?;

        let params = action.params.clone().unwrap_or_default();
        Ok(params)
    }
    
    // Execute a CPI action
    pub fn execute(
        &self,
        provider_name: &str,
        action_name: &str,
        params: HashMap<String, Value>,
    ) -> Result<Value, CpiError> {
        let provider = self.get_provider(provider_name)?;
        info!(
            "Executing action '{}' from provider '{}'",
            action_name, provider_name
        );
        let start = std::time::Instant::now();

        let result = time("Executing CPI actions", || {
            executor::execute_action(&provider, action_name, params)
        });

        let duration = start.elapsed();
        if result.is_ok() {
            info!(
                "Action '{}' from provider '{}' completed successfully in {:?}",
                action_name, provider_name, duration
            );
        } else {
            error!(
                "Action '{}' from provider '{}' failed after {:?}",
                action_name, provider_name, duration
            );
        }

        result
    }

    // Optimized helper method to get a provider
    fn get_provider(&self, provider_name: &str) -> Result<Provider, CpiError> {
        match self.providers.get(provider_name) {
            Some(provider) => Ok(provider.clone()),
            None => {
                // Avoid collecting all keys unless needed
                let err_msg = format!(
                    "Provider '{}' not found. Available providers: {}",
                    provider_name, 
                    self.providers.iter().map(|e| e.key().clone()).collect::<Vec<_>>().join(", ")
                );
                error!("{}", err_msg);
                Err(CpiError::ProviderNotFound(provider_name.to_string()))
            }
        }
    }
}

// Helper function for JSON error location not needed in optimized path
fn find_json_error_location(content: &str, error: &serde_json::Error) -> Option<(usize, usize)> {
    let line = error.line();
    let column = error.column();
    if line > 0 && column > 0 {
        Some((line, column))
    } else {
        None
    }
}
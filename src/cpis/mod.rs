// mod.rs - Complete improved initialization with better logging

pub mod error;
pub mod executor;
pub mod loader;
pub mod logger;
pub mod parser;
pub mod provider;
pub mod validator;

use crate::cpis;

use self::error::CpiError;
use self::provider::Provider;
use self::validator::validate_cpi_format;
use dashmap::DashMap;
use log::{debug, error, info, trace, warn};
use rayon::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

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
    logger::configure_from_env();

    let mut system = CpiSystem::new();
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
    pub fn load_all_providers(&mut self) -> Result<usize, CpiError> {
        info!("Loading all CPI providers from ./CPIs directory");

        // Load CPIs using the loader module
        let cpis = loader::load_cpis();

        let binding = cpis.iter().collect::<Vec<_>>();
        let valid_cpis = binding.par_iter()
            .filter_map(|cpi| {
                println!("Validating provider file: {:?}", cpi.key());

                match serde_json::from_str::<Value>(cpi.value()) {
                    Ok(json_value) => {
                        self.validate_provider_file(PathBuf::from(cpi.key()), json_value.to_string()).ok();
                        return Some(cpi.key()) // Return the key if validation and registration are successful;
                    }
                    Err(e) => {
                        error!("Failed to parse JSON for CPI '{}': {}", cpi.key(), e);
                        None::<&str>
                    }
                };

                Some(cpi.key())
            })
            .collect::<Vec<_>>();

        println!("Found {} valid CPIs", valid_cpis.len());

        // Register each valid provider
        for cpi in valid_cpis.iter() {
            let provider_content = cpis.get(*cpi).unwrap().value().clone();
            if let Err(e) = self.register_provider(cpi.to_string(), provider_content, false) {
                error!("Failed to register provider '{}': {}", cpi, e);
            } else {
                info!("Successfully registered provider '{}'", cpi);
            }
        }

        let loaded_count = self.providers.len() as usize;
        if loaded_count == 0 {
            return Err(CpiError::NoProvidersLoaded);
        }
        // Report successful loading with clear formatting for visibility
        info!("============================================");
        let total_files: usize = cpis.len();
        info!(
            "✅ Successfully loaded {}/{} CPI providers",
            loaded_count,
            total_files
        );
        info!("============================================");

        // Return the list of failed provider names for better debugging
        info!("Failed to load {} providers", total_files - loaded_count);
        if total_files > loaded_count {
            warn!("Some providers failed to load. Check logs for details.");
        }

        Ok(loaded_count)
    }

    // Enhanced validation with better error reporting
    fn validate_provider_file(&self, path: PathBuf, cpi_content: String) -> Result<(), CpiError> {
        info!("Validating CPI file: {}", path.display());

        // Try to parse the JSON
        let json: Value = match serde_json::from_str(&cpi_content) {
            Ok(json) => {
                info!("Successfully parsed JSON from file: {}", path.display());
                json
            }
            Err(e) => {
                let err_msg = format!("Failed to parse JSON in file: {}", e);

                // Provide more details about parse errors
                let line_col = find_json_error_location(&cpi_content, &e);
                if let Some((line, col)) = line_col {
                    error!("JSON error at line {}, column {}", line, col);
                    if let Some(problematic_line) = cpi_content.lines().nth(line - 1) {
                        error!("Problematic line: {}", problematic_line);
                        error!("{}^", " ".repeat(col - 1));
                    }
                }

                return Err(CpiError::SerdeError(e));
            }
        };

        // Validate the JSON structure
        match path.to_str() {
            Some(path_str) => validate_cpi_format(path_str, &json),
            None => Err(CpiError::InvalidPath("Invalid path".to_string())),
        }
    }

    // Enhanced provider registration with better error reporting
    pub fn register_provider(
        &mut self,
        provider_name: String,
        provider_content: String,
        should_test: bool,
    ) -> Result<(), CpiError> {
        //map string to provider struct using serde
        let provider: Provider = serde_json::from_str(&provider_content).map_err(|e| {
            dbg!(&e);
            error!("Failed to parse JSON for provider {} Error: {}", provider_name, e);
            CpiError::SerdeError(e)
        })?;
        info!("Registering provider: {:?}", provider.name);

        if should_test {
            info!("Running test command on provider '{}'", provider.name);
            let test_result = executor::execute_action(&provider, "test_install", HashMap::new());

            match test_result {
                Ok(_) => {
                    info!("Test command succeeded for provider '{}'", provider.name);
                }
                Err(e) => {
                    error!(
                        "Test command failed for provider '{}': {}",
                        provider.name, e
                    );
                    return Err(e);
                }
            }
        }

        self.providers
            .insert(provider.name.clone(), provider);
        Ok(())
    }

    // Get available providers
    pub fn get_providers(&self) -> Vec<String> {
        let providers = self.providers.iter().map(|entry| entry.key().clone()).collect();
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
    pub fn get_action_params(
        &self,
        provider_name: &str,
        action_name: &str,
    ) -> Result<Vec<String>, CpiError> {
        let provider = self.get_provider(provider_name)?;
        let action = provider.get_action(action_name)?;

        let params = if let Some(params) = &action.params {
            params.clone()
        } else {
            Vec::new()
        };

        debug!(
            "Parameters for action '{}' in provider '{}': {:?}",
            action_name, provider_name, params
        );

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
        if let Ok(_) = &result {
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

    // Helper method to get a provider
    fn get_provider(&self, provider_name: &str) -> Result<Provider, CpiError> {
        match self.providers.get(provider_name) {
            Some(provider) => Ok(provider.clone()),
            None => {
            let available = self
                .providers
                .iter()
                .map(|entry| entry.key().clone())
                .collect::<Vec<_>>()
                .join(", ");
            let err_msg = format!(
                "Provider '{}' not found. Available providers: {}",
                provider_name, available
            );
            error!("{}", err_msg);
            Err(CpiError::ProviderNotFound(provider_name.to_string()))
            }
        }
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

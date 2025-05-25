//! # CPI System Module
//!
//! The CPI (Component Provider Interface) system provides a modular framework for
//! loading, validating, and executing actions from various providers.
//!
//! This module implements a thread-safe, concurrent system for managing CPI providers,
//! handling initialization, registration, validation, and execution of provider actions.
//!
//! ## Key Features
//!
//! * Thread-safe provider management using concurrent maps
//! * Parallel loading and validation of providers
//! * Error tracking and reporting
//! * Action execution with parameter validation
//!
//! ## Usage
//!
//! ```rust
//! // Initialize the CPI system
//! let cpi_system = omni_director::cpis::initialize()?;
//!
//! // Execute an action on a provider
//! let result = cpi_system.execute("provider_name", "action_name", params)?;
//! ```

pub mod error;
pub mod executor;
pub mod extensions;
pub mod loader;
pub mod logger;
pub mod parser;
pub mod provider;
pub mod validator;

use self::error::CpiError;
use self::extensions::ExtensionManager;
use self::provider::Provider;
use self::validator::validate_cpi_format;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use rayon::prelude::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};
use std::sync::atomic::{AtomicBool, Ordering};

/// Ensures the initialization happens exactly once
static INIT: Once = Once::new();
/// Tracks whether the system has been fully initialized
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Measures and logs the execution time of a function in debug mode
///
/// # Arguments
///
/// * `name` - The name of the action being timed, used in the debug log
/// * `f` - The function to time
///
/// # Returns
///
/// The result of the function execution
///
/// # Examples
///
/// ```
/// let result = time("Loading providers", || load_all_providers());
/// ```
#[cfg(debug_assertions)]
fn time<T, A: ToString, F: FnOnce() -> T>(name: A, f: F) -> T {
    let time = std::time::Instant::now();
    let out = f();
    let action = name.to_string();
    debug!("{} Took {:?}", action, time.elapsed());
    out
}

/// No-op version of the time function for release builds
///
/// Simply executes the function without timing in non-debug builds
#[cfg(not(debug_assertions))]
fn time<T, A: ToString, F: FnOnce() -> T>(_: A, f: F) -> T {
    f()
}

/// Initializes the CPI system and loads all providers
///
/// This function handles the initialization of the CPI system, ensuring that
/// it only happens once, even when called from multiple threads. It configures 
/// logging, loads all providers, and returns a usable CPI system instance.
///
/// # Returns
///
/// * `Result<CpiSystem, CpiError>` - A Result containing either a fully initialized 
///   CPI system or an error if initialization failed
///
/// # Examples
///
/// ```
/// let cpi_system = match cpis::initialize() {
///     Ok(system) => system,
///     Err(e) => {
///         error!("Failed to initialize CPI system: {}", e);
///         return Err(e.into());
///     }
/// };
/// ```
pub fn initialize() -> Result<CpiSystem, error::CpiError> {
    // Skip initialization if already done
    if INITIALIZED.load(Ordering::SeqCst) {
        debug!("CPI system already initialized, reusing existing instance");
        return Ok(CpiSystem::new());
    }

    info!("Initializing CPI system");
    let start = std::time::Instant::now();

    // Configure logging based on environment
    let _ = logger::configure_from_env();

    let system = CpiSystem::new();
    
    // Use Once to ensure initialization happens exactly once
    let mut result = Ok(0);
    INIT.call_once(|| {
        match system.load_all_providers_and_extensions() {
            Ok(count) => {
                result = Ok(count);
                INITIALIZED.store(true, Ordering::SeqCst);
            }
            Err(e) => {
                result = Err(e);
            }
        }
    });

    match result {
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

/// Main struct representing the CPI system
///
/// This struct provides the public API for the CPI system, allowing clients to
/// load, query, and execute provider actions. It uses thread-safe data structures
/// to enable concurrent access from multiple threads.
pub struct CpiSystem {
    /// Map of provider names to Provider instances
    providers: Arc<DashMap<String, Provider>>,
    /// Map of provider names to their source file paths
    /// This enables proper error reporting and debugging
    provider_sources: Arc<DashMap<String, String>>,
    /// Extension manager for dynamically loaded extensions
    pub extensions: Arc<ExtensionManager>,
}

impl Clone for CpiSystem {
    /// Creates a clone of the CPI system that shares the same underlying data
    ///
    /// This is an efficient operation as it only increments reference counters
    /// for the Arc-wrapped DashMaps.
    fn clone(&self) -> Self {
        Self {
            providers: Arc::clone(&self.providers),
            provider_sources: Arc::clone(&self.provider_sources),
            extensions: Arc::clone(&self.extensions),
        }
    }
}

// Use a lazy static pattern for the providers to ensure they're shared
lazy_static::lazy_static! {
    /// Global shared map of providers accessible across all CPI system instances
    static ref GLOBAL_PROVIDERS: Arc<DashMap<String, Provider>> = Arc::new(DashMap::new());
    /// Global shared map of provider source files accessible across all CPI system instances
    static ref GLOBAL_PROVIDER_SOURCES: Arc<DashMap<String, String>> = Arc::new(DashMap::new());
    /// Global shared extension manager accessible across all CPI system instances
    static ref GLOBAL_EXTENSIONS: Arc<ExtensionManager> = Arc::new(ExtensionManager::new());
}

impl CpiSystem {
    /// Creates a new CPI system instance
    ///
    /// This creates a new instance that shares the same underlying data
    /// with all other instances, enabling efficient sharing of providers
    /// across multiple parts of the application.
    ///
    /// # Returns
    ///
    /// A new CPI system instance
    ///
    /// # Examples
    ///
    /// ```
    /// let cpi_system = CpiSystem::new();
    /// ```
    pub fn new() -> Self {
        Self {
            providers: Arc::clone(&GLOBAL_PROVIDERS),
            provider_sources: Arc::clone(&GLOBAL_PROVIDER_SOURCES),
            extensions: Arc::clone(&GLOBAL_EXTENSIONS),
        }
    }

    /// Loads all providers from the ./CPIs directory and all extensions from the ./Extensions directory
    ///
    /// This method scans the CPIs and Extensions directories, validates each provider file and extension,
    /// and registers valid providers. It handles errors gracefully and provides
    /// detailed error reporting. The method can be called from multiple threads
    /// safely due to the use of concurrent data structures.
    ///
    /// # Returns
    ///
    /// * `Result<usize, CpiError>` - The number of successfully loaded providers or an error
    ///
    /// # Errors
    ///
    /// * `CpiError::NoProvidersLoaded` - If no valid providers were found
    /// * Other errors from the validation and registration process
    pub fn load_all_providers_and_extensions(&self) -> Result<usize, CpiError> {
        // If providers are already loaded, just return the count
        if !self.providers.is_empty() {
            debug!("Providers already loaded, count: {}", self.providers.len());
            return Ok(self.providers.len());
        }

        info!("Loading all CPI providers from ./CPIs directory");

        // Load CPIs using the loader module
        let cpis = Arc::new(loader::load_cpis());
        let total_cpis = cpis.len();

        info!("Total CPIs found: {}", total_cpis);
        
        // Use a concurrent HashSet for tracking valid CPIs
        let valid_cpis = DashMap::new();
        
        // Process all CPIs in parallel
        let cpis_iter: Vec<_> = cpis.iter().collect();
        cpis_iter.par_iter().for_each(|entry| {
            let cpi_key = entry.key();
            let cpi_value = entry.value();
            
            debug!("Validating provider file: {:?}", cpi_key);

            // Avoid multiple JSON parse operations by parsing once
            match serde_json::from_str::<Value>(cpi_value) {
                Ok(json_value) => {
                    if let Ok(provider) = serde_json::from_str::<Provider>(cpi_value) {
                        // Check for naming collisions using atomic operation
                        info!("Attempting to load CPI: {}...", provider.name);

                        match self.providers.entry(provider.name.clone()) {
                            dashmap::mapref::entry::Entry::Vacant(entry) => {
                                // Store the provider
                                entry.insert(provider.clone());
                                
                                // Store the mapping from provider name to source file
                                self.provider_sources.insert(provider.name.clone(), cpi_key.to_string());

                                info!("Successfully loaded provider: {}", provider.name);
                            },
                            dashmap::mapref::entry::Entry::Occupied(occupied_entry) => {
                                // Get the source file for the existing provider
                                let existing_provider_name = occupied_entry.key().clone();
                                let existing_file = self.provider_sources.get(&existing_provider_name)
                                    .map(|s| s.value().clone())
                                    .unwrap_or_else(|| "unknown file".to_string());

                                let time = chrono::Local::now().format("[%Y-%m-%d %H:%M:%S%.3f]");
                                let prefix = format!("{} [unknown] ✖ ERROR [omni_director::cpis] : ", time);
                                
                                // Format the entire error message with prefixes on each line
                                let error_message = format!(
                                    "------------------------------------------------------------------------\n\
                                     {0}\x1b[1;31mNaming collision detected for CPI '{1}'\n\
                                     {0}The colliding CPI can be found in file: {2:?}\n\
                                     {0}The already registered CPI can be found in file: {3:?}\n\
                                     {0}Please edit the CPI name within the file to avoid conflicts\n\
                                     {0}Each CPI must have a unique name in its file under the \"name\" key\n\
                                     {0}Skipping this CPI to prevent conflicts\x1b[0m\n\
                                     {0}------------------------------------------------------------------------",
                                    prefix, provider.name, cpi_key, existing_file
                                );
                        
                                // Log the entire message in one call to prevent threading issues
                                error!("{}", error_message);
                                return;
                            }
                        }

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

        // Now load extensions
        info!("Loading extensions from ./Extensions directory");
        let extensions_dir = PathBuf::from("./Extensions");
        
        if extensions_dir.exists() && extensions_dir.is_dir() {
            match self.extensions.load_all_extensions(&extensions_dir) {
                Ok(count) => {
                    info!("Loaded {} extensions", count);
                    
                    // Register extensions as providers
                    for ext_name in self.extensions.get_extensions() {
                        match self.extensions.extension_to_provider(&ext_name) {
                            Ok(provider) => {
                                // Register the provider
                                self.providers.insert(provider.name.clone(), provider.clone());
                                self.provider_sources.insert(provider.name.clone(), format!("extension:{}", ext_name));
                                info!("Registered extension '{}' as provider", ext_name);
                            },
                            Err(e) => {
                                error!("Failed to convert extension '{}' to provider: {}", ext_name, e);
                            }
                        }
                    }
                },
                Err(e) => {
                    warn!("Failed to load extensions: {}", e);
                }
            }
        } else {
            debug!("Extensions directory not found, skipping extension loading");
        }

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
            warn!("\x1b[31m❌ Failed to load {} providers:\x1b[0m", failed_cpis.len());
            for failed in &failed_cpis {
                warn!("   \x1b[31m- {}\x1b[0m", failed);
            }
            warn!("===============================================");
        }
        
        info!("🎉 Successfully validated: {}/{} providers", valid_cpis.len(), total_cpis);
        info!("============================================");

        Ok(loaded_count)
    }

    /// Loads all providers from the ./CPIs directory
    ///
    /// This method is kept for backward compatibility but delegates to the new method
    pub fn load_all_providers(&self) -> Result<usize, CpiError> {
        self.load_all_providers_and_extensions()
    }

    /// Validates a provider file using a pre-parsed JSON value
    ///
    /// This is an optimized validation method that accepts a pre-parsed JSON value
    /// to avoid parsing the same content multiple times.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the provider file
    /// * `json` - The pre-parsed JSON value
    ///
    /// # Returns
    ///
    /// * `Result<(), CpiError>` - Ok if validation passed, or an error
    ///
    /// # Errors
    ///
    /// * `CpiError::InvalidPath` - If the path is invalid
    /// * Other errors from the validation process
    fn validate_provider_file(&self, path: PathBuf, json: &Value) -> Result<(), CpiError> {
        debug!("Validating CPI file: {}", path.display());

        // Validate the JSON structure
        match path.to_str() {
            Some(path_str) => validate_cpi_format(path_str, json),
            None => Err(CpiError::InvalidPath("Invalid path".to_string())),
        }
    }

    /// Registers a provider directly with optimized processing
    ///
    /// This is a more efficient version of provider registration that avoids
    /// redundant operations when the provider has already been parsed and validated.
    ///
    /// # Arguments
    ///
    /// * `provider_file` - The source file path for the provider
    /// * `provider` - The already parsed Provider instance
    ///
    /// # Returns
    ///
    /// * `Result<(), CpiError>` - Ok if registration succeeded, or an error
    fn register_provider_direct(
        &self,
        provider_file: String,
        provider: Provider,
    ) -> Result<(), CpiError> {
        debug!("Registering provider: {:?}", provider.name);
        
        // Store the mapping from provider name to source file
        self.provider_sources.insert(provider.name.clone(), provider_file);
        
        // Store the provider itself
        self.providers.insert(provider.name.clone(), provider);
        
        Ok(())
    }

    /// Registers a provider with optional testing
    ///
    /// This method parses a provider from its JSON content, optionally runs a test
    /// command, and registers it in the system. It provides enhanced error reporting
    /// for better diagnostics.
    ///
    /// # Arguments
    ///
    /// * `provider_name` - The name

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
            executor::execute_action(Arc::new(self.clone()), &provider, action_name, params)
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
    
    // Get the source file for a provider
    pub fn get_provider_source(&self, provider_name: &str) -> Option<String> {
        self.provider_sources.get(provider_name).map(|s| s.value().clone())
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
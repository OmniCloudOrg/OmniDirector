pub mod parser;
pub mod executor;
pub mod provider;
pub mod error;
pub mod validator;

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs::{self, File};
use serde_json::Value;
use self::error::CpiError;
use self::provider::Provider;
use self::validator::validate_cpi_format;

pub fn initialize() -> Result<CpiSystem, error::CpiError> {
    let mut system = CpiSystem::new();
    system.load_all_providers()?;
    Ok(system)
}


// Public API for the CPI system
pub struct CpiSystem {
    providers: HashMap<String, Provider>,
}

impl CpiSystem {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }
    
    // Load all providers from the ./CPIs directory
    pub fn load_all_providers(&mut self) -> Result<(), CpiError> {
        let cpi_dir = Path::new("./CPIs");
        
        if !cpi_dir.exists() {
            return Err(CpiError::InvalidPath("./CPIs directory does not exist".to_string()));
        }
        
        if !cpi_dir.is_dir() {
            return Err(CpiError::InvalidPath("./CPIs is not a directory".to_string()));
        }
        
        let entries = fs::read_dir(cpi_dir)
            .map_err(|e| CpiError::IoError(e))?;
        
        let mut loaded_count = 0;
        
        for entry in entries {
            let entry = entry.map_err(|e| CpiError::IoError(e))?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                // Validate the JSON before loading
                if let Err(e) = self.validate_provider_file(&path) {
                    eprintln!("Warning: Failed to validate CPI file {:?}: {}", path, e);
                    continue;
                }
                
                // Load the provider
                match self.register_provider(path.clone()) {
                    Ok(_) => {
                        loaded_count += 1;
                    },
                    Err(e) => {
                        eprintln!("Warning: Failed to load CPI file {:?}: {}", path, e);
                    }
                }
            }
        }
        
        if loaded_count == 0 {
            return Err(CpiError::NoProvidersLoaded);
        }
        
        Ok(())
    }

    // Validate a provider file format before loading
    fn validate_provider_file(&self, path: &Path) -> Result<(), CpiError> {
        let file = File::open(path)
            .map_err(|e| CpiError::IoError(e))?;
        
        let json: Value = serde_json::from_reader(file)
            .map_err(|e| CpiError::SerdeError(e))?;
        
        validate_cpi_format(&json)
    }

    // Register a provider from a JSON file path
    pub fn register_provider(&mut self, path: PathBuf) -> Result<(), CpiError> {
        let provider = provider::load_provider(path)?;
        self.providers.insert(provider.name.clone(), provider);
        Ok(())
    }

    // Get available providers
    pub fn get_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    // Get available actions for a provider
    pub fn get_provider_actions(&self, provider_name: &str) -> Result<Vec<String>, CpiError> {
        let provider = self.get_provider(provider_name)?;
        Ok(provider.actions.keys().cloned().collect())
    }

    // Get the required parameters for an action
    pub fn get_action_params(&self, provider_name: &str, action_name: &str) -> Result<Vec<String>, CpiError> {
        let provider = self.get_provider(provider_name)?;
        let action = provider.get_action(action_name)?;
        
        if let Some(params) = &action.params {
            Ok(params.clone())
        } else {
            Ok(Vec::new())
        }
    }

    // Execute a CPI action
    pub fn execute(&self, provider_name: &str, action_name: &str, params: HashMap<String, Value>) -> Result<Value, CpiError> {
        let provider = self.get_provider(provider_name)?;
        executor::execute_action(provider, action_name, params)
    }

    // Helper method to get a provider
    fn get_provider(&self, provider_name: &str) -> Result<&Provider, CpiError> {
        self.providers.get(provider_name)
            .ok_or_else(|| CpiError::ProviderNotFound(provider_name.to_string()))
    }
}

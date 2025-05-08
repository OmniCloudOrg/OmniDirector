use crate::cpis::error::CpiError;
use crate::cpis::provider::{ActionDef, ActionTarget, ParseRules, Provider};
use lib_cpi::{ActionDefinition, CpiExtension, GetExtensionFn, ParamType};
use libloading::{Library, Symbol};
use log::{debug, error, info, trace, warn};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::ffi::OsStr;

type ExtensionPtr = *mut dyn CpiExtension;
type LoadedExtension = Box<dyn CpiExtension>;

// Structure to keep track of loaded extensions and libraries
pub struct ExtensionManager {
    // We need to keep the libraries loaded while we use the extensions
    libraries: Mutex<Vec<Library>>,
    extensions: Mutex<HashMap<String, LoadedExtension>>,
}

impl ExtensionManager {
    pub fn new() -> Self {
        Self {
            libraries: Mutex::new(Vec::new()),
            extensions: Mutex::new(HashMap::new()),
        }
    }
    
    // Load all extensions from the specified directory
    pub fn load_all_extensions(&self, dir: &Path) -> Result<usize, CpiError> {
        if !dir.exists() || !dir.is_dir() {
            return Err(CpiError::InvalidPath(format!(
                "Extension directory does not exist: {:?}",
                dir
            )));
        }
        
        let entries = std::fs::read_dir(dir).map_err(|e| {
            error!("Failed to read extension directory {:?}: {}", dir, e);
            CpiError::IoError(e)
        })?;
        
        let mut count = 0;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };
            
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            
            // Check if this is a valid library file
            #[cfg(target_os = "windows")]
            let is_lib = path.extension() == Some(OsStr::new("dll"));
            
            #[cfg(target_os = "linux")]
            let is_lib = path.extension() == Some(OsStr::new("so"));
            
            #[cfg(target_os = "macos")]
            let is_lib = path.extension() == Some(OsStr::new("dylib"));
            
            if !is_lib {
                continue;
            }
            
            // Try to load the extension
            match self.load_extension(&path) {
                Ok(()) => count += 1,
                Err(e) => {
                    warn!("Failed to load extension from {:?}: {}", path, e);
                }
            }
        }
        
        info!("Successfully loaded {} extensions", count);
        Ok(count)
    }
    
    // Load a single extension from the specified path
    pub fn load_extension(&self, path: &Path) -> Result<(), CpiError> {
        debug!("Loading extension from: {:?}", path);
        
        // Attempt to load the library
        let lib = unsafe {
            Library::new(path).map_err(|e| {
                error!("Failed to load library {:?}: {}", path, e);
                CpiError::InvalidCpiFormat(format!("Failed to load library: {}", e))
            })?
        };
        
        // Get the extension function
        let get_extension: Symbol<GetExtensionFn> = unsafe {
            lib.get(b"get_extension").map_err(|e| {
                error!("Failed to find get_extension function in {:?}: {}", path, e);
                CpiError::InvalidCpiFormat(format!("Failed to find get_extension function: {}", e))
            })?
        };
        
        // Call the function to get the extension
        let extension_ptr = unsafe { get_extension() };
        
        // Convert the raw pointer into a Box
        let extension = unsafe { Box::from_raw(extension_ptr) };
        
        // Get extension info
        let extension_name = extension.name().to_string();
        let provider_type = extension.provider_type().to_string();
        
        info!("Loaded extension: {} (type: {})", extension_name, provider_type);
        
        // Store the library and extension
        {
            let mut libraries = self.libraries.lock().unwrap();
            libraries.push(lib);
        }
        
        {
            let mut extensions = self.extensions.lock().unwrap();
            if extensions.contains_key(&extension_name) {
                error!("Extension with name '{}' already loaded", extension_name);
                return Err(CpiError::InvalidCpiFormat(format!(
                    "Extension with name '{}' already loaded", extension_name
                )));
            }
            
            extensions.insert(extension_name, extension);
        }
        
        Ok(())
    }
    
    // Get a list of loaded extension names
    pub fn get_extensions(&self) -> Vec<String> {
        let extensions = self.extensions.lock().unwrap();
        extensions.keys().cloned().collect()
    }
    
    // Check if an extension is loaded
    pub fn has_extension(&self, name: &str) -> bool {
        let extensions = self.extensions.lock().unwrap();
        extensions.contains_key(name)
    }
    
    // Convert an extension to a Provider
    pub fn extension_to_provider(&self, name: &str) -> Result<Provider, CpiError> {
        let extensions = self.extensions.lock().unwrap();
        let extension = extensions.get(name).ok_or_else(|| {
            error!("Extension not found: {}", name);
            CpiError::ProviderNotFound(name.to_string())
        })?;
        
        // Get all actions from the extension
        let action_names = extension.list_actions();
        let mut actions = HashMap::new();
        
        // Convert actions to the Provider's format
        for action_name in action_names {
            if let Some(action_def) = extension.get_action_definition(&action_name) {
                let action = self.convert_action_definition(action_def, action_name.clone())?;
                actions.insert(action_name, action);
            }
        }
        
        // Convert default settings
        let default_settings = extension.default_settings();
        
        // Create the provider
        let provider = Provider {
            name: name.to_string(),
            provider_type: extension.provider_type().to_string(),
            actions,
            default_settings: if default_settings.is_empty() { None } else { Some(default_settings) },
        };
        
        Ok(provider)
    }
    
    // Helper method to convert ActionDefinition to ActionDef
    fn convert_action_definition(&self, def: ActionDefinition, action_name: String) -> Result<ActionDef, CpiError> {
        // Extract parameter names
        let params = def.parameters.iter()
            .filter(|p| p.required)
            .map(|p| p.name.clone())
            .collect();
        
        // Create a target that delegates to the extension
        let target = ActionTarget::Command(
            crate::cpis::provider::Command { 
                command: format!("__extension__:{}", action_name),
                in_vm: Some(false),
            }
        );
        
        // Create a simple parse rule that returns the result directly
        let parse_rules = ParseRules::Object { 
            patterns: HashMap::new()
        };
        
        Ok(ActionDef {
            target,
            params: Some(params),
            pre_exec: None,
            post_exec: None,
            parse_rules,
        })
    }
    
    // Execute an action on an extension
    pub fn execute_action(&self, extension_name: &str, action: &str, params: HashMap<String, Value>) -> Result<Value, CpiError> {
        let extensions = self.extensions.lock().unwrap();
        let extension = extensions.get(extension_name).ok_or_else(|| {
            error!("Extension not found: {}", extension_name);
            CpiError::ProviderNotFound(extension_name.to_string())
        })?;

        println!("Executing action '{}' on extension '{}'", action, extension_name);
        
        // Execute the action
        extension.execute_action(action, &params).map_err(|e| {
            error!("Extension execution failed: {}", e);
            CpiError::ExecutionFailed(e)
        })
    }
}

// Extension function to determine if a command is for an extension
pub fn is_extension_command(command: &str) -> bool {
    command.starts_with("__extension__:")
}

// Extension function to extract the action name from a command
pub fn extract_extension_action(command: &str) -> Option<&str> {
    if command.starts_with("__extension__:") {
        Some(&command[14..]) // 13 is the length of "__extension__:"
    } else {
        None
    }
}
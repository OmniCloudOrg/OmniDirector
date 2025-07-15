//! # Feature Registration System
//!
//! Provides a clean, macro-based feature registration system similar to the CPI system.
//! This eliminates boilerplate and provides type-safe operation definitions.

use std::collections::HashMap;
use serde_json::Value;
use std::ffi::{CString, CStr};
use std::os::raw::c_char;

/// Trait for feature operations with type-safe execution
pub trait FeatureOperation: Send + Sync {
    /// Get the operation name
    fn name(&self) -> &'static str;
    
    /// Get the required arguments with their types
    fn arguments(&self) -> Vec<(&'static str, &'static str)>;
    
    /// Execute the operation with the provided arguments
    fn execute(&self, args: HashMap<String, Value>) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
}

/// Trait for feature providers
pub trait FeatureProvider: Send + Sync {
    /// Get the feature name
    fn name(&self) -> &'static str;
    
    /// Get all available operations
    fn operations(&self) -> Vec<&'static str>;
    
    /// Execute an operation by name
    fn execute_operation(&self, operation: &str, args: HashMap<String, Value>) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>;
}

/// Wrapper for feature providers to handle FFI
pub struct FeatureWrapper {
    provider: Box<dyn FeatureProvider>,
}

impl FeatureWrapper {
    pub fn new(provider: Box<dyn FeatureProvider>) -> Self {
        Self { provider }
    }
    
    pub fn name(&self) -> &str {
        self.provider.name()
    }
    
    pub fn operations(&self) -> Vec<&str> {
        self.provider.operations()
    }
    
    pub fn execute(&self, operation: &str, args: HashMap<String, Value>) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.provider.execute_operation(operation, args)
    }
}

/// Macro to define a feature with operations
#[macro_export]
macro_rules! define_feature {
    (
        $feature_name:ident {
            $(
                $op_name:ident {
                    args: [$($arg_name:ident: $arg_type:ty),* $(,)?]
                    $(, handler: $handler:expr)?
                }
            ),* $(,)?
        }
    ) => {
        // Generate operation enums
        #[derive(Debug, Clone)]
        pub enum $feature_name {
            $(
                $op_name($($arg_type),*),
            )*
        }
        
        impl $feature_name {
            /// Get operation name as string
            pub fn operation_name(&self) -> &'static str {
                match self {
                    $(
                        $feature_name::$op_name(_) => stringify!($op_name),
                    )*
                }
            }
            
            /// Execute the operation
            pub fn execute(&self, _args: HashMap<String, serde_json::Value>) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
                match self {
                    $(
                        $feature_name::$op_name($($arg_name),*) => {
                            // Default implementation - can be overridden by handler
                            $(
                                if let Some(handler) = $handler {
                                    return handler($($arg_name.clone()),*);
                                }
                            )?
                            
                            // Default success response
                            Ok(serde_json::json!({
                                "operation": stringify!($op_name),
                                "status": "success",
                                "result": {
                                    $(
                                        stringify!($arg_name): $arg_name
                                    ),*
                                }
                            }))
                        }
                    )*
                }
            }
        }
        
        // Generate feature provider implementation
        pub struct [<$feature_name Provider>] {
            name: &'static str,
        }
        
        impl [<$feature_name Provider>] {
            pub fn new() -> Self {
                Self {
                    name: stringify!($feature_name),
                }
            }
        }
        
        impl $crate::cpis::feature_registration::FeatureProvider for [<$feature_name Provider>] {
            fn name(&self) -> &'static str {
                self.name
            }
            
            fn operations(&self) -> Vec<&'static str> {
                vec![
                    $(
                        stringify!($op_name),
                    )*
                ]
            }
            
            fn execute_operation(&self, operation: &str, args: HashMap<String, serde_json::Value>) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
                match operation {
                    $(
                        stringify!($op_name) => {
                            // Extract arguments with type conversion
                            $(
                                let $arg_name = args.get(stringify!($arg_name))
                                    .ok_or(format!("Missing argument: {}", stringify!($arg_name)))?;
                                let $arg_name: $arg_type = serde_json::from_value($arg_name.clone())?;
                            )*
                            
                            let operation = $feature_name::$op_name($($arg_name),*);
                            operation.execute(args)
                        }
                    )*
                    _ => Err(format!("Unknown operation: {}", operation).into())
                }
            }
        }
        
        // Generate FFI functions
        paste::paste! {
            #[no_mangle]
            pub extern "C" fn get_feature_name() -> *const std::os::raw::c_char {
                let name = stringify!($feature_name);
                match std::ffi::CString::new(name) {
                    Ok(c_string) => c_string.into_raw(),
                    Err(_) => std::ptr::null(),
                }
            }
            
            #[no_mangle]
            pub extern "C" fn register_operations() -> *const std::os::raw::c_char {
                let operations = vec![
                    $(
                        stringify!($op_name),
                    )*
                ];
                
                match serde_json::to_string(&operations) {
                    Ok(json) => match std::ffi::CString::new(json) {
                        Ok(c_string) => c_string.into_raw(),
                        Err(_) => std::ptr::null(),
                    },
                    Err(_) => std::ptr::null(),
                }
            }
            
            #[no_mangle]
            pub extern "C" fn create_feature() -> *mut $crate::cpis::feature_registration::FeatureWrapper {
                let provider = Box::new([<$feature_name Provider>]::new());
                let wrapper = $crate::cpis::feature_registration::FeatureWrapper::new(provider);
                Box::into_raw(Box::new(wrapper))
            }
            
            #[no_mangle]
            pub extern "C" fn execute_operation(
                operation_name: *const std::os::raw::c_char,
                args_json: *const std::os::raw::c_char,
            ) -> *const std::os::raw::c_char {
                if operation_name.is_null() || args_json.is_null() {
                    return std::ptr::null();
                }
                
                let operation_name = match unsafe { std::ffi::CStr::from_ptr(operation_name) }.to_str() {
                    Ok(s) => s,
                    Err(_) => return std::ptr::null(),
                };
                
                let args_json = match unsafe { std::ffi::CStr::from_ptr(args_json) }.to_str() {
                    Ok(s) => s,
                    Err(_) => return std::ptr::null(),
                };
                
                let args: HashMap<String, serde_json::Value> = match serde_json::from_str(args_json) {
                    Ok(args) => args,
                    Err(_) => HashMap::new(),
                };
                
                let provider = [<$feature_name Provider>]::new();
                let result = match provider.execute_operation(operation_name, args) {
                    Ok(result) => result,
                    Err(e) => serde_json::json!({ "error": e.to_string() })
                };
                
                match serde_json::to_string(&result) {
                    Ok(json) => match std::ffi::CString::new(json) {
                        Ok(c_string) => c_string.into_raw(),
                        Err(_) => std::ptr::null(),
                    },
                    Err(_) => std::ptr::null(),
                }
            }
            
            #[no_mangle]
            pub extern "C" fn cleanup_feature(wrapper: *mut $crate::cpis::feature_registration::FeatureWrapper) {
                if !wrapper.is_null() {
                    unsafe { Box::from_raw(wrapper) };
                }
            }
            
            #[no_mangle]
            pub extern "C" fn cleanup_string(ptr: *mut std::os::raw::c_char) {
                if !ptr.is_null() {
                    unsafe { std::ffi::CString::from_raw(ptr) };
                }
            }
        }
    };
}

/// Simplified registration macro for features
#[macro_export]
macro_rules! register_feature {
    ($feature_type:ty) => {
        use $crate::cpis::feature_registration::*;
        
        #[no_mangle]
        pub extern "C" fn create_feature() -> *mut FeatureWrapper {
            let provider = Box::new(<$feature_type>::new());
            let wrapper = FeatureWrapper::new(provider);
            Box::into_raw(Box::new(wrapper))
        }
    };
}
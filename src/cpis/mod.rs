//! ---------------------------------------------------------------------------
//! OmniDirector CPI loader
//! ---------------------------------------------------------------------------
//! Authors: Tristan Poland, Maxine DeAndreade
//! ---------------------------------------------------------------------------
//! # About the module
//! 
//! The OmniDirector CPI module is responsible for managing the Cloud Provider
//! Interfaces (CPIs) used by the OmniDirector platform component. It provides
//! functions for loading, validating, and managing CPIs, as well as handling
//! errors and logging. It also sets up a query system via which the api module
//! can query the CPIs for information regarding metadata versioning, registerd
//! actions, required parameters, and other relevant information.
//! 
//! ## CPIs
//! 
//! CPIs are meant to be implemented in Rust as a library and compiled into a
//! shared object, DLL, or similar. The loader will load the shared object and
//! integrate the CPI with the OmniDirector of which will then be able to use the
//! CPI to perform actions on a given provider.
//! 
//! CPIs are expected to implement the `Default` trait and provide a `new`
//! function which will be called by the loader to create an instance of the
//! CPI. The loader will also provide a `get_metadata` function which will
//! be called by the OmniDirector to get metadata about the CPI.
//! 
//! ### Capabilities
//! 
//! CPIs can then register `capabilities` in its new function which will mean
//! that the CPI is capable of performing a given set of actions. If a
//! `capability` is enabled in a CPI then the CPI will be required to implement
//! actions for that capability. The loader will also provide a `get_capabilities`
//! function which will be called by the OmniDirector to get the capabilities
//! 
//! Examples of some capabilities are:
//! - `cpu_worker`
//! - `gpu_worker`
//! - `volume`
//! - `network_extend`

use std::path::Path;

pub mod rt;
pub mod loader;
pub mod error;
pub mod prelude;

const CPI_DIR: &str = "CPIs";

pub fn initialize() {
    // Initialize the CPI system
    let cpi_path = Path::new(CPI_DIR);
    let cpis = loader::load_all_extensions(cpi_path);   
}
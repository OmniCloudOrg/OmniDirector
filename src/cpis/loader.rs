//! ---------------------------------------------------------------------------
//! OmniDirector CPI loader
//! ---------------------------------------------------------------------------
//! Authors: Tristan Poland, Maxine DeAndreade
//! ---------------------------------------------------------------------------
//! The OmniDirector CPI loader is responsible for loading all CPIs (Cloud
//! Provider Interfaces) at platorm startup. It manages the validation,
//! collision detection, and registration of CPIs. The loader also provides
//! logging and error handling for the loading process.
//! ---------------------------------------------------------------------------

use super::error::CpiError;
use super::prelude::*;

use dashmap::DashMap;
use lazy_static::lazy_static;
use log::{error, info, warn};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use comfy_table::modifiers::{UTF8_ROUND_CORNERS, UTF8_SOLID_INNER_BORDERS};
use comfy_table::Color;
use comfy_table::{presets, Cell, ContentArrangement, Row, Table};

/// Represents a loaded CPI extension
#[derive(Debug)]
pub struct LoadedExtension {
    pub name: String,
    pub path: PathBuf,
    pub library: libloading::Library,
}

lazy_static! {
    static ref LOADED_EXTENSIONS: DashMap<String, LoadedExtension> = DashMap::new();
}

const CPI_DIR: &str = "CPIs";

/// Returns the dynamic library extension for the current platform
pub fn get_library_extension() -> &'static str {
    #[cfg(target_os = "windows")]
    return ".dll";

    #[cfg(target_os = "macos")]
    return ".dylib";

    #[cfg(all(unix, not(target_os = "macos")))]
    return ".so";

    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    compile_error!("Unsupported target OS for dynamic library extension");
}

/// Loads all CPI extensions from the specified directory
///
/// Creates the CPI directory if it doesn't exist, then scans for dynamic libraries
/// matching the current platform's extension and attempts to load each one.
///
/// # Arguments
/// * `cpi_dir` - Path to the directory containing CPI libraries
///
/// # Returns
/// * `Result<()>` - Ok if directory processing completes, Err on IO failures
pub fn load_all_extensions(cpi_dir: &Path) -> Result<()> {
    info!("Loading CPIs from directory: {:?}", cpi_dir);

    if !cpi_dir.exists() {
        std::fs::create_dir_all(cpi_dir).map_err(|e| {
            error!("Failed to create CPI directory {:?}: {}", cpi_dir, e);
            CpiError::IoError(e)
        })?;
        info!("Created CPI directory: {:?}", cpi_dir);
    }

    let entries = std::fs::read_dir(cpi_dir).map_err(|e| {
        error!("Failed to read CPI directory {:?}: {}", cpi_dir, e);
        CpiError::IoError(e)
    })?;

    let target_extension = get_library_extension();
    let mut loaded_count = 0;
    let mut failed_count = 0;

    for entry in entries {
        info!("Processing entry: {:?}", entry);
        match entry {
            Ok(entry) => {
                let path = entry.path();

                if path.is_file() && has_target_extension(&path, target_extension) {
                    info!("\x1b[34mCPI will load: {:?}\x1b[0m", entry.file_name());

                    match load_extension(&path) {
                        Ok(_) => {
                            loaded_count += 1;
                            info!("Successfully loaded CPI: {:?}", path);
                        }
                        Err(e) => {
                            failed_count += 1;
                            warn!("Failed to load CPI {:?}: {}", path, e);
                        }
                    }
                } else {
                    warn!(
                        "\x1b[33mCPI will not load: {:?} - {}\x1b[0m",
                        entry.file_name(),
                        if !path.is_file() {
                            "Not a file"
                        } else {
                            "Wrong extension"
                        }
                    );
                }
            }
            Err(e) => {
                warn!("Failed to read directory entry: {}", e);
            }
        }
    }

    // Collect provider names and their loading status
    let mut provider_statuses: Vec<(String, bool)> = Vec::new();

    // Create a nicely formatted table for the loading results
    let mut table = Table::new();
    table
        .set_header(vec![
            Cell::new("CPI PROVIDER")
                .fg(Color::Cyan)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new("STATUS")
                .fg(Color::Cyan)
                .add_attribute(comfy_table::Attribute::Bold),
        ])
        .apply_modifier(UTF8_ROUND_CORNERS)
        .apply_modifier(UTF8_SOLID_INNER_BORDERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .load_preset(presets::UTF8_FULL)
        .set_width(60);

    // Add each successfully loaded provider to the table
    let mut has_loaded_providers = false;
    for entry in LOADED_EXTENSIONS.iter() {
        table.add_row(Row::from(vec![
            Cell::new(entry.key()),
            Cell::new("✅ LOADED").fg(Color::Green),
        ]));
        provider_statuses.push((entry.key().clone(), true));
        has_loaded_providers = true;
    }

    // If no providers were loaded, add a message indicating this
    if !has_loaded_providers {
        table.add_row(Row::from(vec![
            Cell::new("No CPIs loaded successfully").fg(Color::Yellow),
            Cell::new("❌ NONE").fg(Color::Red),
        ]));
    }

    // Add a clear separator before summary
    table.add_row(Row::from(vec![
        Cell::new("").fg(Color::White),
        Cell::new("").fg(Color::White),
    ]));

    // Add summary section header
    table.add_row(Row::from(vec![
        Cell::new("SUMMARY")
            .fg(Color::Yellow)
            .add_attribute(comfy_table::Attribute::Bold),
        Cell::new("").fg(Color::Yellow),
    ]));

    // Add summary rows
    table.add_row(Row::from(vec![
        Cell::new("✅ Successfully Loaded:"),
        Cell::new(&loaded_count.to_string()).fg(Color::Green),
    ]));

    table.add_row(Row::from(vec![
        Cell::new("❌ Failed to Load:"),
        Cell::new(&failed_count.to_string()).fg(Color::Red),
    ]));

    table.add_row(Row::from(vec![
        Cell::new("📊 Total Processed:"),
        Cell::new(&(loaded_count + failed_count).to_string()).fg(Color::Blue),
    ]));

    info!("\n{}", table);

    // You'll need to add this to your Cargo.toml:
    // comfy-table = "6.1.0"
    Ok(())
}

/// Checks if a file has the target dynamic library extension
fn has_target_extension(path: &Path, target_ext: &str) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| format!(".{}", ext) == target_ext)
        .unwrap_or(false)
}

// --------------------------------------------------------------------------------------- //
// --     __  __                  ____          ____                                    -- //
// --    / / / /__  ________     / __ )___     / __ \_________ _____ _____  ____  _____ -- //
// --   / /_/ / _ \/ ___/ _ \   / __  / _ \   / / / / ___/ __ `/ __ `/ __ \/ __ \/ ___/ -- //
// --  / __  /  __/ /  /  __/  / /_/ /  __/  / /_/ / /  / /_/ / /_/ / /_/ / / / (__  )  -- //
// -- /_/ /_/\___/_/   \___/  /_____/\___/  /_____/_/   \__,_/\__, /\____/_/ /_/____/   -- //
// --                                                        /____/                     -- //
// --------------------------------------------------------------------------------------- //

/// Loads a single CPI extension from the given path
///
/// # Arguments
/// * `path` - Full path to the dynamic library file
///
/// # Returns
/// * `Result<()>` - Ok if loading succeeds, Err on any failure
///
/// # Safety
/// This function loads and executes code from dynamic libraries, which is inherently unsafe.
/// The loaded libraries must implement the expected CPI interface correctly.
///
/// # Warning
/// ⚠️ This implementation is not production-ready. It lacks proper validation and testing.
///
#[doc(hidden)]
#[deprecated(
    since = "0.1.4",
    note = "This function is experimental and may change or be removed at any time"
)]
fn load_extension(path: &Path) -> Result<()> {
    let name = path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| CpiError::InvalidPath(path.to_string_lossy().to_string()))?
        .to_string();

    // Check for naming collisions
    if LOADED_EXTENSIONS.contains_key(&name) {
        return Err(CpiError::CpiNameCollision(name));
    }

    // Load the dynamic library
    let library = unsafe {
        libloading::Library::new(path).map_err(|e| {
            error!("Failed to load library {:?}: {}", path, e);
            CpiError::LibraryLoadingError(e)
        })?
    };

    // Call the initialization function
    unsafe {
        let init_fn: libloading::Symbol<unsafe extern "C" fn() -> i32> =
            library.get(b"cpi_init").map_err(|e| {
                error!("Failed to find cpi_init function in {:?}: {}", path, e);
                CpiError::MalformedCpiError(name.clone())
            })?;

        let result = init_fn();
        if result != 0 {
            // Convert std::io::Error to anyhow::Error
            let io_error = std::io::Error::from_raw_os_error(result);
            return Err(CpiError::InitializationFailedError(
                name,
                anyhow::Error::new(io_error),
            ));
        }
    }

    // Store the loaded extension
    let extension = LoadedExtension {
        name: name.clone(),
        path: path.to_path_buf(),
        library,
    };

    LOADED_EXTENSIONS.insert(name.clone(), extension);
    info!("Registered CPI extension: {}", name);

    Ok(())
}

/// Loads all CPIs from the default directory
pub fn load_default_cpis() -> Result<()> {
    let cpi_path = PathBuf::from(CPI_DIR);
    load_all_extensions(&cpi_path)
}

/// Returns the names of all currently loaded extensions
pub fn get_loaded_extension_names() -> Vec<String> {
    LOADED_EXTENSIONS
        .iter()
        .map(|entry| entry.key().clone())
        .collect()
}

/// Checks if a specific extension is loaded
pub fn is_extension_loaded(name: &str) -> bool {
    LOADED_EXTENSIONS.contains_key(name)
}

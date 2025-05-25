mod api;
mod cpis;
mod logging;

pub mod proposal;

use anyhow::Result;
use env_logger::{Builder, Target, WriteStyle};
use log::LevelFilter;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use chrono::Utc;
use regex::Regex;

struct DualWriter {
    stdout: io::Stdout,
    file: Arc<Mutex<std::fs::File>>,
    ansi_regex: Regex,
}

impl DualWriter {
    fn new(log_file_path: &str) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file_path)?;
        
        // Regex to match ANSI escape sequences
        let ansi_regex = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        
        Ok(DualWriter {
            stdout: io::stdout(),
            file: Arc::new(Mutex::new(file)),
            ansi_regex,
        })
    }
    
    fn strip_ansi_codes(&self, text: &str) -> String {
        self.ansi_regex.replace_all(text, "").to_string()
    }
}

impl Write for DualWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Write to stdout with ANSI codes intact
        self.stdout.write_all(buf)?;
        
        // Strip ANSI codes and write to file
        if let Ok(text) = std::str::from_utf8(buf) {
            let clean_text = self.strip_ansi_codes(text);
            if let Ok(mut file) = self.file.lock() {
                file.write_all(clean_text.as_bytes())?;
                file.flush()?;
            }
        }
        
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()?;
        if let Ok(mut file) = self.file.lock() {
            file.flush()?;
        }
        Ok(())
    }
}

pub fn init_logger() -> io::Result<()> {
    // Get app name from Cargo
    let app_name = env!("CARGO_PKG_NAME");
    
    // Create logs directory if it doesn't exist
    let logs_dir = "logs";
    if !Path::new(logs_dir).exists() {
        fs::create_dir_all(logs_dir)?;
    }
    
    // Create a properly named log file with timestamp
    let timestamp = Utc::now().format("%Y-%m-%d_%H-%M-%S");
    let log_file_path = format!("{}/{}_{}.log", logs_dir, app_name, timestamp);
    
    // Create the dual writer
    let dual_writer = DualWriter::new(&log_file_path)?;
    
    // Initialize the logger with custom target and force colors
    Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp_secs()
        .write_style(WriteStyle::Always)  // Force ANSI codes even for non-TTY
        .target(Target::Pipe(Box::new(dual_writer)))
        .init();
    
    println!("Logger initialized. Writing to stdout and {}", log_file_path);
    Ok(())
}

// In case we just want file output
pub fn init_logger_file_only() -> io::Result<()> {
    // Get app name from Cargo
    let app_name = env!("CARGO_PKG_NAME");
    
    // Create logs directory if it doesn't exist
    let logs_dir = "logs";
    if !Path::new(logs_dir).exists() {
        fs::create_dir_all(logs_dir)?;
    }
    
    let timestamp = Utc::now().format("%Y-%m-%d_%H-%M-%S");
    let log_file_path = format!("{}/{}_{}.log", logs_dir, app_name, timestamp);
    
    let target = Box::new(std::fs::File::create(&log_file_path)?);
    
    Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp_secs()
        .write_style(WriteStyle::Always)  // Force ANSI codes
        .target(Target::Pipe(target))
        .init();
    
    println!("Logger initialized. Writing to {}", log_file_path);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger
    init_logger();

    // Initialize the CPI system
    println!("Initializing CPI system...");
    let cpi_system = cpis::initialize();

    // List available providers
    // TODO: @tristanpoland - Uncomment this when the cpi_system is implemented
    println!("Available providers:");
    // for provider in cpi_system.get_providers() {
    //     println!("  - {}", provider);
    // }

    //let input_dir: &str = "./";
    //try_compile(input_dir).expect("Could not compile");
    api::launch_rocket().await;
    Ok(())
}

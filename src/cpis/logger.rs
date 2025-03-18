// logger.rs - Enhanced logging with colors and symbols

use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use std::io::{self, Write};
use std::env;
use chrono::Local;

// Re-export standard log macros (for compatibility)
pub use log::{debug, error, info, trace, warn};

// Custom macro definitions for colored logging
#[macro_export]
macro_rules! cinfo {
    ($($arg:tt)*) => {{
        $crate::logger::log_with_color(log::Level::Info, format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! cwarning {
    ($($arg:tt)*) => {{
        $crate::logger::log_with_color(log::Level::Warn, format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! cerror {
    ($($arg:tt)*) => {{
        $crate::logger::log_with_color(log::Level::Error, format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! cdebug {
    ($($arg:tt)*) => {{
        $crate::logger::log_with_color(log::Level::Debug, format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! ctrace {
    ($($arg:tt)*) => {{
        $crate::logger::log_with_color(log::Level::Trace, format_args!($($arg)*));
    }};
}

// Helper function used by macros
pub fn log_with_color(level: Level, args: std::fmt::Arguments) {
    if level <= log::max_level() {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let (color, symbol) = match level {
            Level::Error => (RED, ERROR_SYMBOL),
            Level::Warn => (YELLOW, WARN_SYMBOL),
            Level::Info => (GREEN, INFO_SYMBOL),
            Level::Debug => (BLUE, DEBUG_SYMBOL),
            Level::Trace => (MAGENTA, TRACE_SYMBOL),
        };
        
        let target = module_path!();
        let thread_name = std::thread::current().name().unwrap_or("unknown").to_string();
        
        // Format: [timestamp] [thread] SYMBOL LEVEL target: message
        writeln!(
            io::stderr(),
            "{BOLD}[{}]{RESET} [{thread_name}] {color}{symbol} {}{RESET} {}{}: {}",
            now,
            level,
            CYAN,
            target,
            args
        ).ok();
    }
}

// ANSI color codes
const RESET: &str = "\x1B[0m";
const RED: &str = "\x1B[31m";
const GREEN: &str = "\x1B[32m";
const YELLOW: &str = "\x1B[33m";
const BLUE: &str = "\x1B[34m";
const MAGENTA: &str = "\x1B[35m";
const CYAN: &str = "\x1B[36m";
const BOLD: &str = "\x1B[1m";

// Log level symbols
const ERROR_SYMBOL: &str = "✖";
const WARN_SYMBOL: &str = "⚠";
const INFO_SYMBOL: &str = "ℹ";
const DEBUG_SYMBOL: &str = "🔍";
const TRACE_SYMBOL: &str = "➤";

struct ColorLogger;

impl log::Log for ColorLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let (color, symbol) = match record.level() {
                Level::Error => (RED, ERROR_SYMBOL),
                Level::Warn => (YELLOW, WARN_SYMBOL),
                Level::Info => (GREEN, INFO_SYMBOL),
                Level::Debug => (BLUE, DEBUG_SYMBOL),
                Level::Trace => (MAGENTA, TRACE_SYMBOL),
            };
            
            let target = if !record.target().is_empty() {
                format!("[{}] ", record.target())
            } else {
                String::new()
            };
            
            let thread_name = std::thread::current().name().unwrap_or("unknown").to_string();
            
            // Format: [timestamp] [thread] SYMBOL LEVEL target: message
            writeln!(
                io::stderr(),
                "{BOLD}[{}]{RESET} [{thread_name}] {color}{symbol} {}{RESET} {}{}: {}",
                now,
                record.level(),
                CYAN,
                target,
                record.args()
            ).ok();
        }
    }

    fn flush(&self) {
        io::stderr().flush().ok();
    }
}

static LOGGER: ColorLogger = ColorLogger;

pub fn configure_from_env() -> Result<(), SetLoggerError> {
    // Check for no-color flag or environment variable
    let use_colors = !env::var("NO_COLOR").is_ok() && !env::args().any(|arg| arg == "--no-color");
    
    // Check for log level from environment or use default
    let log_level = match env::var("RUST_LOG").ok() {
        Some(level) => match level.to_lowercase().as_str() {
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "info" => LevelFilter::Info,
            "debug" => LevelFilter::Debug,
            "trace" => LevelFilter::Trace,
            _ => LevelFilter::Info,
        },
        None => LevelFilter::Info,
    };
    
    // Initialize the global logger
    log::set_logger(&LOGGER).map(|()| log::set_max_level(log_level))
}

// Additional helper function to create a progress bar
pub fn create_progress_bar(total: usize) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new(total as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-")
    );
    pb
}

// Success/failure indicators for operations
pub fn log_success(message: &str) {
    println!("{}✓{} {}{}{}", GREEN, RESET, GREEN, message, RESET);
}

pub fn log_failure(message: &str) {
    println!("{}✗{} {}{}{}", RED, RESET, RED, message, RESET);
}

// For group operations with multiple steps
pub struct GroupLogger {
    name: String,
    start_time: std::time::Instant,
}

impl GroupLogger {
    pub fn start(name: &str) -> Self {
        println!("{}▼{} Starting: {}", BLUE, RESET, name);
        Self {
            name: name.to_string(),
            start_time: std::time::Instant::now(),
        }
    }
    
    pub fn end(self) {
        let duration = self.start_time.elapsed();
        println!("{}▲{} Completed: {} in {:?}", BLUE, RESET, self.name, duration);
    }
    
    pub fn step(&self, message: &str) {
        println!("{}│{} {}", CYAN, RESET, message);
    }
    
    pub fn success_step(&self, message: &str) {
        println!("{}│{} {}✓{} {}", CYAN, RESET, GREEN, RESET, message);
    }
    
    pub fn warn_step(&self, message: &str) {
        println!("{}│{} {}⚠{} {}", CYAN, RESET, YELLOW, RESET, message);
    }
    
    pub fn error_step(&self, message: &str) {
        println!("{}│{} {}✖{} {}", CYAN, RESET, RED, RESET, message);
    }
}
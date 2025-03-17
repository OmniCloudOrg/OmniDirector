// logger.rs - Structured logging for CPI system
use std::sync::Once;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use chrono::Local;
use std::io::{self, Write};

// Simple logger implementation
struct CpiLogger;

static LOGGER_INIT: Once = Once::new();
static LOGGER: CpiLogger = CpiLogger;

impl log::Log for CpiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let level = record.level();
            let target = record.target();
            let file = record.file().unwrap_or("unknown");
            let line = record.line().unwrap_or(0);
            
            let mut stderr = io::stderr();
            let _ = writeln!(
                stderr,
                "{} [{:<5}] [{}:{}] {}: {}",
                now, level, file, line, target, record.args()
            );
        }
    }

    fn flush(&self) {
        let _ = io::stderr().flush();
    }
}

// Initialize the logger
pub fn init(level: LevelFilter) -> Result<(), SetLoggerError> {
    let mut result = Ok(());
    
    LOGGER_INIT.call_once(|| {
        result = log::set_logger(&LOGGER)
            .map(|()| log::set_max_level(level));
    });
    
    result
}

// Convenience functions for logging
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        log::trace!($($arg)*)
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        log::debug!($($arg)*)
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        log::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        log::warn!($($arg)*)
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        log::error!($($arg)*)
    };
}

// Configure the logging level based on environment variables
pub fn configure_from_env() {
    let level = match std::env::var("CPI_LOG_LEVEL").unwrap_or_default().to_uppercase().as_str() {
        "TRACE" => LevelFilter::Trace,
        "DEBUG" => LevelFilter::Debug,
        "INFO" => LevelFilter::Info,
        "WARN" => LevelFilter::Warn,
        "ERROR" => LevelFilter::Error,
        _ => LevelFilter::Info, // Default level
    };
    
    if let Err(e) = init(level) {
        eprintln!("Failed to initialize logger: {}", e);
    }
}
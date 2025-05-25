use log::LevelFilter;
use chrono::Local;
use std::fs::OpenOptions;
use std::io::{Write, stdout};

/// A writer that writes to both stdout and a file
struct MultiWriter {
    file: std::fs::File,
}

impl MultiWriter {
    fn new(file: std::fs::File) -> Self {
        Self { file }
    }
}

impl Write for MultiWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Write to stdout
        stdout().write_all(buf)?;
        stdout().flush()?;
        
        // Write to file
        self.file.write_all(buf)?;
        self.file.flush()?;
        
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        stdout().flush()?;
        self.file.flush()
    }
}

/// Initializes the logger for the application
pub fn init_logger() {
    // Create log directory if it doesn't exist
    std::fs::create_dir_all("logs").unwrap_or_default();
    
    // Generate log file name with timestamp
    let log_file_name = format!("logs/omnidirector_{}.log", Local::now().format("%Y-%m-%d_%H-%M-%S"));
    
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(log_file_name)
        .expect("Failed to open log file");
    
    let multi_writer = MultiWriter::new(file);
    
    // Create a builder for the combined logger
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp_secs()
        .write_style(env_logger::WriteStyle::Always) // Force colors
        .format(|buf, record| {
            let timestamp = buf.timestamp_seconds();
            writeln!(
                buf,
                "[{}] {} - {}",
                timestamp,
                record.level(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(Box::new(multi_writer)))
        .init();
}
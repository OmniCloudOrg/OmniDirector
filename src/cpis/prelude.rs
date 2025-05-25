pub use super::error::CpiError;

pub type Result<T> = std::result::Result<T, CpiError>;
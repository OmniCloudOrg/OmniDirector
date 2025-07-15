//! # New API Layer
//!
//! Clean API layer with consistent routing and naming conventions.

pub mod handlers;
pub mod server;
pub mod middleware;
pub mod responses;

pub use handlers::*;
pub use server::*;
pub use middleware::*;
pub use responses::*;
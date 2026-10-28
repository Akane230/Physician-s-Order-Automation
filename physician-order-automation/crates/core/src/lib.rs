//! Shared, local-first business logic for the Physician's Order application.
//!
//! Keep Tauri and UI concerns out of this crate so the same rules can be reused by
//! a future LAN service.

pub mod auth;
pub mod db;
pub mod error;
pub mod parsing;
pub mod patients;

pub use error::{CoreError, CoreResult};

/// Kept temporarily for the starter Tauri command while the command surface is built.
pub fn ping() -> String {
    "orders_core is alive".to_string()
}

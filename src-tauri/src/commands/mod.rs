//! Tauri command handlers for the Aether-Core backend.
//!
//! This module is the thin IPC bridge layer. Tauri commands are
//! registered here and delegate to the business logic modules.

pub mod audit_commands;
pub mod capture_commands;
pub mod network_commands;

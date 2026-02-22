//! Aether-Core error types.
//!
//! Provides a unified error enum for all backend operations,
//! with automatic conversion to serializable formats for the Tauri IPC bridge.

use serde::Serialize;
use thiserror::Error;

/// Primary error type for all Aether-Core operations.
#[derive(Debug, Error)]
pub enum AetherError {
    /// A shell command failed to execute or returned a non-zero exit code.
    #[error("Command execution failed: {command} — {detail}")]
    CommandFailed { command: String, detail: String },

    /// The operation requires elevated privileges (root/sudo).
    #[error("Permission denied: {0}. Run with elevated privileges (sudo/pkexec).")]
    PermissionDenied(String),

    /// The requested network interface does not exist on this system.
    #[error("Interface not found: '{0}'")]
    InterfaceNotFound(String),

    /// Failed to parse output from a system command.
    #[error("Parse error: {origin} — {detail}")]
    ParseError { origin: String, detail: String },

    /// The current operating system does not support this operation.
    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    /// Packet capture error (pcap layer).
    #[error("Capture error: {0}")]
    CaptureError(String),

    /// A capture session is already running on this interface.
    #[error("Capture already running on interface '{0}'")]
    CaptureAlreadyRunning(String),

    /// Wrapper for standard I/O errors.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Serializable error payload sent to the React frontend via Tauri IPC.
#[derive(Debug, Serialize, Clone)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

impl From<&AetherError> for ErrorPayload {
    fn from(err: &AetherError) -> Self {
        let code = match err {
            AetherError::CommandFailed { .. } => "COMMAND_FAILED",
            AetherError::PermissionDenied(_) => "PERMISSION_DENIED",
            AetherError::InterfaceNotFound(_) => "INTERFACE_NOT_FOUND",
            AetherError::ParseError { .. } => "PARSE_ERROR",
            AetherError::UnsupportedPlatform(_) => "UNSUPPORTED_PLATFORM",
            AetherError::CaptureError(_) => "CAPTURE_ERROR",
            AetherError::CaptureAlreadyRunning(_) => "CAPTURE_ALREADY_RUNNING",
            AetherError::Io(_) => "IO_ERROR",
        }
        .to_string();

        ErrorPayload {
            code,
            message: err.to_string(),
        }
    }
}

// Allow AetherError to be returned from Tauri commands.
// Tauri requires command return errors to implement Serialize.
impl Serialize for AetherError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let payload = ErrorPayload::from(self);
        payload.serialize(serializer)
    }
}

pub type AetherResult<T> = Result<T, AetherError>;

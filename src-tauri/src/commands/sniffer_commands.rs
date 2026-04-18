//! Tauri IPC command handlers for the frame sniffer.
//!
//! Provides `start_sniffer` and `stop_sniffer` commands that control
//! the probe request and deauth detection engine. Runs in parallel
//! with the main beacon capture (airodump-ng) using its own pcap handle.

use std::sync::Mutex;

use log::info;
use tauri::Emitter;

use crate::error::AetherError;
use crate::network::sniffer::{SnifferConfig, SnifferHandle};
use crate::network::CaptureStatus;

/// Managed Tauri state holding the active sniffer session.
pub struct SnifferState(pub Mutex<Option<SnifferHandle>>);

/// Start the frame sniffer for probe requests and deauth detection.
///
/// Runs alongside the main beacon capture on the same monitor-mode interface.
/// Emits `"probe-request"` and `"deauth-detected"` Tauri events.
#[tauri::command]
pub fn start_sniffer(
    interface_name: String,
    probe_requests: Option<bool>,
    deauth_frames: Option<bool>,
    state: tauri::State<'_, SnifferState>,
    app_handle: tauri::AppHandle,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_sniffer: {}", interface_name);

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("sniffer".into()));
    }

    let config = SnifferConfig {
        probe_requests: probe_requests.unwrap_or(true),
        deauth_frames: deauth_frames.unwrap_or(true),
    };

    let probe_handle = app_handle.clone();
    let probe_iface = interface_name.clone();
    let deauth_handle = app_handle.clone();
    let deauth_iface = interface_name.clone();

    let handle = crate::network::sniffer::start_sniffer(
        &interface_name,
        config,
        move |probe| {
            if let Err(e) = probe_handle.emit("probe-request", &probe) {
                log::warn!("Failed to emit probe-request on '{}': {}", probe_iface, e);
            }
        },
        move |deauth| {
            if let Err(e) = deauth_handle.emit("deauth-detected", &deauth) {
                log::warn!(
                    "Failed to emit deauth-detected on '{}': {}",
                    deauth_iface,
                    e
                );
            }
        },
    )?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Sniffer started. Probe requests and deauth frames are being monitored.".into(),
    })
}

/// Stop the active sniffer session.
#[tauri::command]
pub fn stop_sniffer(
    state: tauri::State<'_, SnifferState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_sniffer");

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    match guard.take() {
        Some(mut handle) => {
            handle.stop();
            Ok(CaptureStatus {
                interface_name: String::new(),
                active: false,
                message: "Sniffer stopped.".into(),
            })
        }
        None => Ok(CaptureStatus {
            interface_name: String::new(),
            active: false,
            message: "No sniffer was running.".into(),
        }),
    }
}

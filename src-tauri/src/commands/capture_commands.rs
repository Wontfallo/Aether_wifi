//! Tauri IPC command handlers for packet capture operations.
//!
//! Provides `start_capture` and `stop_capture` commands that the React
//! frontend invokes to control the packet sniffing engine.
//!
//! The capture runs in a background thread. Parsed beacon frames are
//! streamed to the frontend via Tauri's event system as `"beacon-frame"` events.

use std::sync::Mutex;

use log::info;
use tauri::Emitter;

use crate::error::AetherError;
use crate::network::{self, CaptureHandle, CaptureStatus};

/// Managed Tauri state holding the active capture session (if any).
///
/// We use `Mutex<Option<CaptureHandle>>` so that:
///   - `None` = no capture running
///   - `Some(handle)` = active capture session
///
/// Only one capture can run at a time (per the WiFi adapter constraint).
pub struct CaptureState(pub Mutex<Option<CaptureHandle>>);

/// Start capturing beacon frames on a monitor-mode interface.
///
/// Parsed beacon data is emitted as Tauri events with name `"beacon-frame"`
/// and a JSON payload matching the `BeaconFrame` struct.
///
/// # Frontend Usage
/// ```typescript
/// import { invoke } from '@tauri-apps/api/core';
/// import { listen } from '@tauri-apps/api/event';
///
/// // Start listening for beacons
/// const unlisten = await listen('beacon-frame', (event) => {
///   const beacon = event.payload; // { bssid, ssid, channel, rssi, ... }
///   console.log('Beacon:', beacon);
/// });
///
/// // Start the capture
/// await invoke('start_capture', { interfaceName: 'wlan0mon' });
///
/// // Later, stop the capture
/// await invoke('stop_capture');
/// unlisten();
/// ```
#[tauri::command]
pub fn start_capture(
    interface_name: String,
    state: tauri::State<'_, CaptureState>,
    app_handle: tauri::AppHandle,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_capture: {}", interface_name);

    let mut capture_guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if capture_guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning(
            interface_name.clone(),
        ));
    }

    // Clone the app handle for use in the capture callback.
    // This closure is called on the capture thread for each parsed beacon.
    let handle = app_handle.clone();
    let iface_clone = interface_name.clone();

    let capture_handle = network::packet_sniffer::start_capture(
        &interface_name,
        move |beacon| {
            // Emit the beacon frame as a Tauri event to all frontend windows
            if let Err(e) = handle.emit("beacon-frame", &beacon) {
                log::warn!(
                    "Failed to emit beacon-frame event on '{}': {}",
                    iface_clone,
                    e
                );
            }
        },
    )?;

    *capture_guard = Some(capture_handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Capture started. Beacon frames are being streamed to the frontend.".into(),
    })
}

/// Stop the active capture session.
///
/// If no capture is running, this is a no-op (returns success with a message).
#[tauri::command]
pub fn stop_capture(
    state: tauri::State<'_, CaptureState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_capture");

    let mut capture_guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    match capture_guard.take() {
        Some(mut handle) => {
            handle.stop();
            Ok(CaptureStatus {
                interface_name: String::new(),
                active: false,
                message: "Capture stopped.".into(),
            })
        }
        None => Ok(CaptureStatus {
            interface_name: String::new(),
            active: false,
            message: "No capture was running.".into(),
        }),
    }
}

/// Check whether a capture is currently active.
#[tauri::command]
pub fn capture_status(
    state: tauri::State<'_, CaptureState>,
) -> Result<CaptureStatus, AetherError> {
    let capture_guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    let active = capture_guard
        .as_ref()
        .map(|h| h.is_active())
        .unwrap_or(false);

    Ok(CaptureStatus {
        interface_name: String::new(),
        active,
        message: if active {
            "Capture is running.".into()
        } else {
            "No active capture.".into()
        },
    })
}

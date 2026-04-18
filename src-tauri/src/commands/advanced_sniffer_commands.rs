//! Tauri IPC command handlers for the advanced sniffer features.
//!
//! Provides start/stop commands for packet monitoring, raw capture,
//! PMKID extraction, pwnagotchi detection, SAE sniffing, and MAC tracking.
//! Each running feature emits Tauri events to the frontend.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use log::info;
use tauri::Emitter;

use crate::error::AetherError;
use crate::network::advanced_sniffer;
use crate::network::CaptureStatus;

/// Shared state for all advanced sniffers. Each feature is tracked by a string key.
pub struct AdvancedSnifferState(pub Mutex<HashMap<String, Arc<AtomicBool>>>);

fn get_or_start(
    state: &Mutex<HashMap<String, Arc<AtomicBool>>>,
    key: &str,
) -> Result<Arc<AtomicBool>, AetherError> {
    let mut guard = state
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.contains_key(key) {
        return Err(AetherError::CaptureAlreadyRunning(key.into()));
    }

    let flag = Arc::new(AtomicBool::new(false));
    guard.insert(key.to_string(), flag.clone());
    Ok(flag)
}

fn stop_feature(
    state: &Mutex<HashMap<String, Arc<AtomicBool>>>,
    key: &str,
) -> Result<CaptureStatus, AetherError> {
    let mut guard = state
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    match guard.remove(key) {
        Some(flag) => {
            flag.store(true, Ordering::SeqCst);
            Ok(CaptureStatus {
                interface_name: String::new(),
                active: false,
                message: format!("{} stopped.", key),
            })
        }
        None => Ok(CaptureStatus {
            interface_name: String::new(),
            active: false,
            message: format!("No {} was running.", key),
        }),
    }
}

// ─────────────────────────────────────────────────
// A. Packet Monitor
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn start_packet_monitor(
    interface_name: String,
    state: tauri::State<'_, AdvancedSnifferState>,
    app_handle: tauri::AppHandle,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_packet_monitor: {}", interface_name);
    let flag = get_or_start(&state.0, "packet_monitor")?;

    let _handle = advanced_sniffer::start_packet_monitor(&interface_name, flag, move |stats| {
        if let Err(e) = app_handle.emit("packet-stats", &stats) {
            log::warn!("Failed to emit packet-stats: {}", e);
        }
    })?;

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Packet monitor started.".into(),
    })
}

#[tauri::command]
pub fn stop_packet_monitor(
    state: tauri::State<'_, AdvancedSnifferState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_packet_monitor");
    stop_feature(&state.0, "packet_monitor")
}

// ─────────────────────────────────────────────────
// B. Raw Capture
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn start_raw_capture(
    interface_name: String,
    state: tauri::State<'_, AdvancedSnifferState>,
    app_handle: tauri::AppHandle,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_raw_capture: {}", interface_name);
    let flag = get_or_start(&state.0, "raw_capture")?;

    let _handle = advanced_sniffer::start_raw_capture(&interface_name, flag, move |frame| {
        if let Err(e) = app_handle.emit("raw-frame", &frame) {
            log::warn!("Failed to emit raw-frame: {}", e);
        }
    })?;

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Raw capture started.".into(),
    })
}

#[tauri::command]
pub fn stop_raw_capture(
    state: tauri::State<'_, AdvancedSnifferState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_raw_capture");
    stop_feature(&state.0, "raw_capture")
}

// ─────────────────────────────────────────────────
// C. PMKID Capture
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn start_pmkid_capture(
    interface_name: String,
    state: tauri::State<'_, AdvancedSnifferState>,
    app_handle: tauri::AppHandle,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_pmkid_capture: {}", interface_name);
    let flag = get_or_start(&state.0, "pmkid_capture")?;

    let _handle = advanced_sniffer::start_pmkid_capture(&interface_name, flag, move |capture| {
        if let Err(e) = app_handle.emit("pmkid-capture", &capture) {
            log::warn!("Failed to emit pmkid-capture: {}", e);
        }
    })?;

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "PMKID capture started.".into(),
    })
}

#[tauri::command]
pub fn stop_pmkid_capture(
    state: tauri::State<'_, AdvancedSnifferState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_pmkid_capture");
    stop_feature(&state.0, "pmkid_capture")
}

// ─────────────────────────────────────────────────
// D. Pwnagotchi Detection
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn detect_pwnagotchi(
    interface_name: String,
    state: tauri::State<'_, AdvancedSnifferState>,
    app_handle: tauri::AppHandle,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] detect_pwnagotchi: {}", interface_name);
    let flag = get_or_start(&state.0, "pwnagotchi_detect")?;

    let _handle =
        advanced_sniffer::start_pwnagotchi_detect(&interface_name, flag, move |info| {
            if let Err(e) = app_handle.emit("pwnagotchi-detected", &info) {
                log::warn!("Failed to emit pwnagotchi-detected: {}", e);
            }
        })?;

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Pwnagotchi detection started.".into(),
    })
}

#[tauri::command]
pub fn stop_pwnagotchi_detect(
    state: tauri::State<'_, AdvancedSnifferState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_pwnagotchi_detect");
    stop_feature(&state.0, "pwnagotchi_detect")
}

// ─────────────────────────────────────────────────
// E. SAE Sniff
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn start_sae_sniff(
    interface_name: String,
    state: tauri::State<'_, AdvancedSnifferState>,
    app_handle: tauri::AppHandle,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_sae_sniff: {}", interface_name);
    let flag = get_or_start(&state.0, "sae_sniff")?;

    let _handle = advanced_sniffer::start_sae_sniff(&interface_name, flag, move |sae| {
        if let Err(e) = app_handle.emit("sae-frame", &sae) {
            log::warn!("Failed to emit sae-frame: {}", e);
        }
    })?;

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "SAE sniff started.".into(),
    })
}

#[tauri::command]
pub fn stop_sae_sniff(
    state: tauri::State<'_, AdvancedSnifferState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_sae_sniff");
    stop_feature(&state.0, "sae_sniff")
}

// ─────────────────────────────────────────────────
// F. MAC Track
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn start_mac_track(
    interface_name: String,
    target_mac: String,
    state: tauri::State<'_, AdvancedSnifferState>,
    app_handle: tauri::AppHandle,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_mac_track: {} on {}", target_mac, interface_name);
    let flag = get_or_start(&state.0, "mac_track")?;

    let _handle =
        advanced_sniffer::start_mac_track(&interface_name, &target_mac, flag, move |entry| {
            if let Err(e) = app_handle.emit("mac-track", &entry) {
                log::warn!("Failed to emit mac-track: {}", e);
            }
        })?;

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: format!("MAC tracking started for {}.", target_mac),
    })
}

#[tauri::command]
pub fn stop_mac_track(
    state: tauri::State<'_, AdvancedSnifferState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_mac_track");
    stop_feature(&state.0, "mac_track")
}

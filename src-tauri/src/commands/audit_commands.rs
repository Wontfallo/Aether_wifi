//! Tauri IPC command handlers for offensive audit operations.
//!
//! Exposes the deauthentication injection and EAPOL handshake capture
//! functions from `network::audit` as Tauri commands invokable by the
//! React frontend.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use log::info;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::error::AetherError;
use crate::network::audit;

// ─────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────

/// Result payload for deauth injection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeauthResult {
    pub success: bool,
    pub message: String,
    pub bssid: String,
    pub packets_sent: u32,
}

/// Result payload for handshake capture operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResult {
    pub success: bool,
    pub message: String,
    pub pcap_path: Option<String>,
}

/// Status for the full "one-click capture" operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureOperationStatus {
    pub phase: String,
    pub message: String,
    pub progress: f32,
}

// ─────────────────────────────────────────────────
// State: tracks active EAPOL capture
// ─────────────────────────────────────────────────

/// Managed Tauri state for the EAPOL handshake capture session.
pub struct EapolCaptureState(pub Mutex<Option<Arc<AtomicBool>>>);

// ─────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────

/// Parse a MAC address string like "AA:BB:CC:DD:EE:FF" into 6 bytes.
fn parse_mac(mac_str: &str) -> Result<[u8; 6], AetherError> {
    let parts: Vec<&str> = mac_str.split(':').collect();
    if parts.len() != 6 {
        return Err(AetherError::CommandFailed {
            command: "parse_mac".into(),
            detail: format!(
                "Invalid MAC address format: '{}'. Expected XX:XX:XX:XX:XX:XX",
                mac_str
            ),
        });
    }

    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] = u8::from_str_radix(part, 16).map_err(|_| AetherError::CommandFailed {
            command: "parse_mac".into(),
            detail: format!("Invalid hex byte '{}' in MAC address '{}'", part, mac_str),
        })?;
    }

    Ok(bytes)
}

/// Send a broadcast deauthentication frame targeting a specific BSSID.
///
/// # Frontend Usage
/// ```typescript
/// const result = await invoke('send_deauth', {
///   interfaceName: 'wlan0mon',
///   bssid: 'AA:BB:CC:DD:EE:FF',
///   count: 5
/// });
/// ```
#[tauri::command]
pub fn send_deauth(
    interface_name: String,
    bssid: String,
    count: Option<u32>,
) -> Result<DeauthResult, AetherError> {
    info!(
        "[cmd] send_deauth: interface={}, bssid={}, count={:?}",
        interface_name, bssid, count
    );

    let mac_bytes = parse_mac(&bssid)?;
    let send_count = count.unwrap_or(3);

    for i in 0..send_count {
        audit::inject_broadcast_deauth(&interface_name, &mac_bytes).map_err(|e| {
            AetherError::CaptureError(format!(
                "Deauth injection failed on packet {}/{}: {}",
                i + 1,
                send_count,
                e
            ))
        })?;
    }

    Ok(DeauthResult {
        success: true,
        message: format!(
            "Transmitted {} broadcast deauth frames targeting {}",
            send_count, bssid
        ),
        bssid,
        packets_sent: send_count,
    })
}

/// Start capturing EAPOL (WPA handshake) packets and saving to a .pcap file.
///
/// Runs in a background thread. The capture can be stopped with `stop_eapol_capture`.
///
/// # Frontend Usage
/// ```typescript
/// const result = await invoke('start_eapol_capture', {
///   interfaceName: 'wlan0mon',
///   outputPath: '/tmp/handshake.pcap'
/// });
/// ```
#[tauri::command]
pub fn start_eapol_capture(
    interface_name: String,
    output_path: Option<String>,
    state: tauri::State<'_, EapolCaptureState>,
    app_handle: tauri::AppHandle,
) -> Result<HandshakeResult, AetherError> {
    info!("[cmd] start_eapol_capture: interface={}", interface_name);

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("EAPOL capture".into()));
    }

    // Generate output path if not provided
    let pcap_path = output_path.unwrap_or_else(|| {
        std::fs::create_dir_all("./captures").ok();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("./captures/aether_handshake_{}.pcap", ts)
    });

    let stop_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&stop_flag);
    let path_clone = pcap_path.clone();
    let iface_clone = interface_name.clone();

    // Emit status events to the frontend
    let handle = app_handle.clone();

    // Spawn the capture in a background thread
    std::thread::spawn(move || {
        // Emit "started" event
        let _ = handle.emit(
            "eapol-status",
            CaptureOperationStatus {
                phase: "capturing".into(),
                message: format!("Listening for EAPOL packets on {}...", iface_clone),
                progress: 0.5,
            },
        );

        match audit::capture_and_save_eapol(&iface_clone, &path_clone, flag_clone) {
            Ok(()) => {
                let _ = handle.emit(
                    "eapol-status",
                    CaptureOperationStatus {
                        phase: "complete".into(),
                        message: format!("Handshake saved to {}", path_clone),
                        progress: 1.0,
                    },
                );
            }
            Err(e) => {
                let _ = handle.emit(
                    "eapol-status",
                    CaptureOperationStatus {
                        phase: "error".into(),
                        message: format!("EAPOL capture error: {}", e),
                        progress: 0.0,
                    },
                );
            }
        }
    });

    *guard = Some(stop_flag);

    Ok(HandshakeResult {
        success: true,
        message: format!("EAPOL capture started. Saving to {}", pcap_path),
        pcap_path: Some(pcap_path),
    })
}

/// Stop the active EAPOL capture session.
#[tauri::command]
pub fn stop_eapol_capture(
    state: tauri::State<'_, EapolCaptureState>,
) -> Result<HandshakeResult, AetherError> {
    info!("[cmd] stop_eapol_capture");

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    match guard.take() {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            Ok(HandshakeResult {
                success: true,
                message: "EAPOL capture stopped.".into(),
                pcap_path: None,
            })
        }
        None => Ok(HandshakeResult {
            success: true,
            message: "No EAPOL capture was running.".into(),
            pcap_path: None,
        }),
    }
}

/// Execute a "one-click" handshake capture sequence:
/// 1. Send deauth frames to force client reconnection
/// 2. Start EAPOL capture to catch the handshake
///
/// This is the "1-Click Capture" feature from the blueprint.
#[tauri::command]
pub fn one_click_capture(
    interface_name: String,
    bssid: String,
    deauth_count: Option<u32>,
    output_path: Option<String>,
    state: tauri::State<'_, EapolCaptureState>,
    app_handle: tauri::AppHandle,
) -> Result<HandshakeResult, AetherError> {
    info!(
        "[cmd] one_click_capture: interface={}, bssid={}",
        interface_name, bssid
    );

    let mac_bytes = parse_mac(&bssid)?;
    let count = deauth_count.unwrap_or(5);

    // Generate output path
    let pcap_path = output_path.unwrap_or_else(|| {
        std::fs::create_dir_all("./captures").ok();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("./captures/aether_{}_{}.pcap", bssid.replace(':', ""), ts)
    });

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("EAPOL capture".into()));
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&stop_flag);
    let path_clone = pcap_path.clone();
    let iface_clone = interface_name.clone();
    let bssid_clone = bssid.clone();
    let handle = app_handle.clone();

    std::thread::spawn(move || {
        // Phase 1: Start EAPOL listener first (so we don't miss the handshake)
        let _ = handle.emit(
            "eapol-status",
            CaptureOperationStatus {
                phase: "setup".into(),
                message: "Arming EAPOL listener...".into(),
                progress: 0.1,
            },
        );

        // Phase 2: Send deauth frames
        let _ = handle.emit(
            "eapol-status",
            CaptureOperationStatus {
                phase: "deauth".into(),
                message: format!("Transmitting {} deauth frames to {}...", count, bssid_clone),
                progress: 0.3,
            },
        );

        for _ in 0..count {
            if let Err(e) = audit::inject_broadcast_deauth(&iface_clone, &mac_bytes) {
                let _ = handle.emit(
                    "eapol-status",
                    CaptureOperationStatus {
                        phase: "error".into(),
                        message: format!("Deauth injection failed: {}", e),
                        progress: 0.0,
                    },
                );
                return;
            }
            // Small delay between deauth bursts
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Phase 3: Listen for EAPOL
        let _ = handle.emit(
            "eapol-status",
            CaptureOperationStatus {
                phase: "capturing".into(),
                message: "Listening for EAPOL handshake packets...".into(),
                progress: 0.5,
            },
        );

        match audit::capture_and_save_eapol(&iface_clone, &path_clone, flag_clone) {
            Ok(()) => {
                let _ = handle.emit(
                    "eapol-status",
                    CaptureOperationStatus {
                        phase: "complete".into(),
                        message: format!("Handshake captured! Saved to {}", path_clone),
                        progress: 1.0,
                    },
                );
            }
            Err(e) => {
                let _ = handle.emit(
                    "eapol-status",
                    CaptureOperationStatus {
                        phase: "error".into(),
                        message: format!("Capture error: {}", e),
                        progress: 0.0,
                    },
                );
            }
        }
    });

    *guard = Some(stop_flag);

    Ok(HandshakeResult {
        success: true,
        message: format!(
            "One-click capture initiated against {}. Saving to {}",
            bssid, pcap_path
        ),
        pcap_path: Some(pcap_path),
    })
}

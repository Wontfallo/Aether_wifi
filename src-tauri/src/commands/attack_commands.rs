//! Tauri IPC command handlers for WiFi attacks.
//!
//! Wraps mdk4 and bettercap to provide Marauder-style attack capabilities:
//! beacon spam, probe flood, evil portal, karma, etc.

use std::sync::Mutex;

use log::info;
use serde::Deserialize;

use crate::error::AetherError;
use crate::network::attack_engine::{
    self, AttackHandle, BeaconSpamMode, BettercapConfig, BettercapHandle,
};
use crate::network::CaptureStatus;

/// Managed Tauri state for the active attack process (mdk4).
pub struct AttackState(pub Mutex<Option<AttackHandle>>);

/// Managed Tauri state for the bettercap daemon.
pub struct BettercapState(pub Mutex<Option<BettercapHandle>>);

// ─────────────────────────────────────────────────
// Beacon Spam
// ─────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeaconSpamType {
    List,
    Random,
    Clone,
    Rickroll,
}

/// Start a beacon spam attack.
///
/// # Parameters
/// - `spam_type`: "list", "random", "clone", or "rickroll"
/// - `ssids`: List of SSIDs (for "list" mode)
/// - `clone_ssid`: SSID to clone (for "clone" mode)
/// - `clone_bssid`: Optional BSSID for clone mode
/// - `channel`: Optional channel to broadcast on
/// - `encryption`: Optional encryption type (n/w/t/a)
#[tauri::command]
pub fn start_beacon_spam(
    interface_name: String,
    spam_type: BeaconSpamType,
    ssids: Option<Vec<String>>,
    clone_ssid: Option<String>,
    clone_bssid: Option<String>,
    channel: Option<u8>,
    encryption: Option<String>,
    state: tauri::State<'_, AttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_beacon_spam on {}", interface_name);

    let mut guard = state.0.lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("attack".into()));
    }

    let mode = match spam_type {
        BeaconSpamType::List => {
            let list = ssids.unwrap_or_default();
            if list.is_empty() {
                return Err(AetherError::CaptureError(
                    "No SSIDs provided for list mode".into(),
                ));
            }
            BeaconSpamMode::List(list)
        }
        BeaconSpamType::Random => BeaconSpamMode::Random,
        BeaconSpamType::Clone => {
            let ssid = clone_ssid.ok_or_else(|| {
                AetherError::CaptureError("No SSID provided for clone mode".into())
            })?;
            BeaconSpamMode::Clone {
                ssid,
                bssid: clone_bssid,
            }
        }
        BeaconSpamType::Rickroll => BeaconSpamMode::RickRoll,
    };

    let handle = attack_engine::start_beacon_spam(
        &interface_name,
        mode,
        channel,
        encryption.as_deref(),
    )?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Beacon spam attack started.".into(),
    })
}

// ─────────────────────────────────────────────────
// Probe Flood
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn start_probe_flood(
    interface_name: String,
    ssid: Option<String>,
    bssid: Option<String>,
    state: tauri::State<'_, AttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_probe_flood on {}", interface_name);

    let mut guard = state.0.lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("attack".into()));
    }

    let handle = attack_engine::start_probe_flood(
        &interface_name,
        ssid.as_deref(),
        bssid.as_deref(),
    )?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Probe flood attack started.".into(),
    })
}

// ─────────────────────────────────────────────────
// MDK4 Deauth (broadcast)
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn start_mdk4_deauth(
    interface_name: String,
    whitelist: Option<Vec<String>>,
    blacklist: Option<Vec<String>>,
    state: tauri::State<'_, AttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_mdk4_deauth on {}", interface_name);

    let mut guard = state.0.lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("attack".into()));
    }

    let handle = attack_engine::start_mdk4_deauth(
        &interface_name,
        whitelist.as_deref(),
        blacklist.as_deref(),
    )?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Broadcast deauth attack started.".into(),
    })
}

// ─────────────────────────────────────────────────
// Stop Attack
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn stop_attack(
    state: tauri::State<'_, AttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_attack");

    let mut guard = state.0.lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    match guard.take() {
        Some(mut handle) => {
            handle.stop();
            Ok(CaptureStatus {
                interface_name: String::new(),
                active: false,
                message: "Attack stopped.".into(),
            })
        }
        None => Ok(CaptureStatus {
            interface_name: String::new(),
            active: false,
            message: "No attack was running.".into(),
        }),
    }
}

// ─────────────────────────────────────────────────
// Bettercap Management
// ─────────────────────────────────────────────────

#[tauri::command]
pub fn start_bettercap_daemon(
    interface_name: String,
    state: tauri::State<'_, BettercapState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_bettercap_daemon on {}", interface_name);

    let mut guard = state.0.lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning("bettercap".into()));
    }

    let handle = attack_engine::start_bettercap(
        &interface_name,
        BettercapConfig::default(),
    )?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Bettercap daemon started with REST API.".into(),
    })
}

#[tauri::command]
pub fn stop_bettercap_daemon(
    state: tauri::State<'_, BettercapState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_bettercap_daemon");

    let mut guard = state.0.lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    match guard.take() {
        Some(mut handle) => {
            handle.stop();
            Ok(CaptureStatus {
                interface_name: String::new(),
                active: false,
                message: "Bettercap daemon stopped.".into(),
            })
        }
        None => Ok(CaptureStatus {
            interface_name: String::new(),
            active: false,
            message: "Bettercap was not running.".into(),
        }),
    }
}

/// Start an evil portal attack (requires bettercap daemon running).
#[tauri::command]
pub fn start_evil_portal(
    ssid: String,
    channel: u8,
    portal_html: Option<String>,
    state: tauri::State<'_, BettercapState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_evil_portal: SSID='{}' CH={}", ssid, channel);

    let guard = state.0.lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    let handle = guard.as_ref().ok_or_else(|| {
        AetherError::CaptureError(
            "Bettercap daemon not running. Start it first with start_bettercap_daemon.".into(),
        )
    })?;

    attack_engine::start_evil_portal(handle, &ssid, channel, portal_html.as_deref())?;

    Ok(CaptureStatus {
        interface_name: String::new(),
        active: true,
        message: format!("Evil portal started: SSID='{}' CH={}", ssid, channel),
    })
}

/// Start a karma attack (requires bettercap daemon running).
#[tauri::command]
pub fn start_karma_attack(
    channel: Option<u8>,
    state: tauri::State<'_, BettercapState>,
) -> Result<CaptureStatus, AetherError> {
    let channel = channel.unwrap_or(6);
    info!("[cmd] start_karma_attack on CH={}", channel);

    let guard = state.0.lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    let handle = guard.as_ref().ok_or_else(|| {
        AetherError::CaptureError(
            "Bettercap daemon not running. Start it first with start_bettercap_daemon.".into(),
        )
    })?;

    attack_engine::start_karma(handle, channel)?;

    Ok(CaptureStatus {
        interface_name: String::new(),
        active: true,
        message: format!("Karma attack started on CH={}", channel),
    })
}

/// Send a raw bettercap command (for advanced users).
#[tauri::command]
pub fn bettercap_command(
    cmd: String,
    state: tauri::State<'_, BettercapState>,
) -> Result<String, AetherError> {
    info!("[cmd] bettercap_command: {}", cmd);

    let guard = state.0.lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    let handle = guard.as_ref().ok_or_else(|| {
        AetherError::CaptureError("Bettercap daemon not running.".into())
    })?;

    handle.run_command(&cmd)
}

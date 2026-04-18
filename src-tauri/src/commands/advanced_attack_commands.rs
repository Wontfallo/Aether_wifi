//! Tauri IPC command handlers for advanced WiFi attacks.
//!
//! Wraps the `advanced_attacks` module to expose CSA injection,
//! association/sleep attack, quiet time, bad message (Michael),
//! and SAE commit flood via the Tauri invoke bridge.

use std::sync::Mutex;

use log::info;

use crate::error::AetherError;
use crate::network::attack_engine::AttackHandle;
use crate::network::advanced_attacks;
use crate::network::CaptureStatus;

/// Managed Tauri state for the active advanced attack process.
pub struct AdvancedAttackState(pub Mutex<Option<AttackHandle>>);

// ─────────────────────────────────────────────────
// Channel Switch Attack (CSA)
// ─────────────────────────────────────────────────

/// Start a Channel Switch Announcement injection attack.
#[tauri::command]
pub fn start_channel_switch(
    interface_name: String,
    target_bssid: String,
    target_channel: u8,
    state: tauri::State<'_, AdvancedAttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!(
        "[cmd] start_channel_switch on {} → BSSID={} CH={}",
        interface_name, target_bssid, target_channel
    );

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning(
            "advanced attack".into(),
        ));
    }

    let handle =
        advanced_attacks::start_channel_switch(&interface_name, &target_bssid, target_channel)?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: format!(
            "CSA attack started: BSSID={} target CH={}",
            target_bssid, target_channel
        ),
    })
}

// ─────────────────────────────────────────────────
// Association / Sleep Attack
// ─────────────────────────────────────────────────

/// Start an association / sleep (auth-DoS) attack.
#[tauri::command]
pub fn start_sleep_attack(
    interface_name: String,
    target_bssid: Option<String>,
    target_mac: Option<String>,
    state: tauri::State<'_, AdvancedAttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] start_sleep_attack on {}", interface_name);

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning(
            "advanced attack".into(),
        ));
    }

    let handle = advanced_attacks::start_sleep_attack(
        &interface_name,
        target_bssid.as_deref(),
        target_mac.as_deref(),
    )?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: "Association/sleep attack started.".into(),
    })
}

// ─────────────────────────────────────────────────
// Quiet Time Attack
// ─────────────────────────────────────────────────

/// Start a Quiet Element IE injection attack.
#[tauri::command]
pub fn start_quiet_time(
    interface_name: String,
    channel: u8,
    duration_ms: u16,
    state: tauri::State<'_, AdvancedAttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!(
        "[cmd] start_quiet_time on {} CH={} dur={}",
        interface_name, channel, duration_ms
    );

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning(
            "advanced attack".into(),
        ));
    }

    let handle = advanced_attacks::start_quiet_time(&interface_name, channel, duration_ms)?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: format!("Quiet time attack started: CH={} dur={}", channel, duration_ms),
    })
}

// ─────────────────────────────────────────────────
// Bad Message Attack (Michael)
// ─────────────────────────────────────────────────

/// Start a bad-message (Michael countermeasure) attack.
#[tauri::command]
pub fn start_bad_message(
    interface_name: String,
    target_bssid: String,
    state: tauri::State<'_, AdvancedAttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!(
        "[cmd] start_bad_message on {} → BSSID={}",
        interface_name, target_bssid
    );

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning(
            "advanced attack".into(),
        ));
    }

    let handle = advanced_attacks::start_bad_message(&interface_name, &target_bssid)?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: format!("Bad message attack started: BSSID={}", target_bssid),
    })
}

// ─────────────────────────────────────────────────
// SAE Commit Flood (WPA3)
// ─────────────────────────────────────────────────

/// Start an SAE commit flood against a WPA3 AP.
#[tauri::command]
pub fn start_sae_flood(
    interface_name: String,
    target_bssid: String,
    state: tauri::State<'_, AdvancedAttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!(
        "[cmd] start_sae_flood on {} → BSSID={}",
        interface_name, target_bssid
    );

    let mut guard = state
        .0
        .lock()
        .map_err(|e| AetherError::CaptureError(format!("Lock poisoned: {}", e)))?;

    if guard.is_some() {
        return Err(AetherError::CaptureAlreadyRunning(
            "advanced attack".into(),
        ));
    }

    let handle = advanced_attacks::start_sae_flood(&interface_name, &target_bssid)?;

    *guard = Some(handle);

    Ok(CaptureStatus {
        interface_name,
        active: true,
        message: format!("SAE commit flood started: BSSID={}", target_bssid),
    })
}

// ─────────────────────────────────────────────────
// Stop
// ─────────────────────────────────────────────────

/// Stop the currently running advanced attack.
#[tauri::command]
pub fn stop_advanced_attack(
    state: tauri::State<'_, AdvancedAttackState>,
) -> Result<CaptureStatus, AetherError> {
    info!("[cmd] stop_advanced_attack");

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
                message: "Advanced attack stopped.".into(),
            })
        }
        None => Ok(CaptureStatus {
            interface_name: String::new(),
            active: false,
            message: "No advanced attack was running.".into(),
        }),
    }
}

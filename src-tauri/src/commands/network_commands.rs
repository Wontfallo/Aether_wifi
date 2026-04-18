//! Tauri IPC command handlers for network operations.
//!
//! These functions are exposed to the React frontend via `tauri::command`.
//! Each handler is a thin wrapper that calls into the `network` module
//! and returns serializable results.

use crate::error::AetherError;
use crate::network::{InterfaceMode, InterfaceModeResult, InterfaceScanner, ModeController, NetworkInterface};

/// Return the monitor-mode interface name.
///
/// Reads `AETHER_MONITOR_IFACE` (set by aether.sh) and falls back to "wlan0".
///
/// Invoked from frontend: `invoke('get_monitor_interface')`
#[tauri::command]
pub fn get_monitor_interface() -> String {
    std::env::var("AETHER_MONITOR_IFACE").unwrap_or_else(|_| "wlan0".to_string())
}

/// List all available network interfaces.
///
/// Returns both wired and wireless interfaces. Each includes
/// metadata like mode, MAC address, UP/DOWN state, and whether
/// it's a wireless adapter.
///
/// Invoked from frontend: `invoke('list_interfaces')`
#[tauri::command]
pub fn list_interfaces() -> Result<Vec<NetworkInterface>, AetherError> {
    log::info!("[cmd] list_interfaces");
    InterfaceScanner::list_interfaces()
}

/// List only wireless network interfaces.
///
/// Filters out wired/virtual interfaces — useful for the UI
/// dropdown that selects a WiFi adapter for scanning.
///
/// Invoked from frontend: `invoke('list_wireless_interfaces')`
#[tauri::command]
pub fn list_wireless_interfaces() -> Result<Vec<NetworkInterface>, AetherError> {
    log::info!("[cmd] list_wireless_interfaces");
    InterfaceScanner::list_wireless()
}

/// Get detailed info for a specific interface by name.
///
/// Invoked from frontend: `invoke('get_interface', { name: 'wlan0' })`
#[tauri::command]
pub fn get_interface(name: String) -> Result<NetworkInterface, AetherError> {
    log::info!("[cmd] get_interface: {}", name);
    InterfaceScanner::get_interface(&name)
}

/// Toggle a wireless interface between Managed and Monitor mode.
///
/// This is the core privileged operation. The sequence:
///   1. Kill interfering processes (NetworkManager, wpa_supplicant)
///   2. Bring interface down
///   3. Set new mode (`iw <iface> set type managed|monitor`)
///   4. Bring interface back up
///   5. Verify the mode actually changed
///
/// Returns an `InterfaceModeResult` with before/after state and success flag.
///
/// Invoked from frontend: `invoke('toggle_interface_mode', { interfaceName: 'wlan0' })`
#[tauri::command]
pub fn toggle_interface_mode(
    interface_name: String,
) -> Result<InterfaceModeResult, AetherError> {
    log::info!("[cmd] toggle_interface_mode: {}", interface_name);
    ModeController::toggle_mode(&interface_name)
}

/// Explicitly set an interface to a specific mode.
///
/// Accepts `target_mode` as "managed" or "monitor".
///
/// Invoked from frontend: `invoke('set_interface_mode', { interfaceName: 'wlan0', targetMode: 'monitor' })`
#[tauri::command]
pub fn set_interface_mode(
    interface_name: String,
    target_mode: String,
) -> Result<InterfaceModeResult, AetherError> {
    log::info!(
        "[cmd] set_interface_mode: {} → {}",
        interface_name,
        target_mode
    );

    let current = InterfaceScanner::get_interface(&interface_name)?;
    let target = InterfaceMode::from_str_lossy(&target_mode);

    if target == InterfaceMode::Unknown {
        return Err(AetherError::CommandFailed {
            command: "set_interface_mode".into(),
            detail: format!(
                "Invalid target mode '{}'. Use 'managed' or 'monitor'.",
                target_mode
            ),
        });
    }

    if current.mode == target {
        return Ok(InterfaceModeResult {
            interface_name: interface_name.clone(),
            previous_mode: current.mode.clone(),
            current_mode: current.mode,
            success: true,
            message: format!("{} is already in {} mode.", interface_name, target),
        });
    }

    ModeController::set_mode(&interface_name, &target)?;

    let updated = InterfaceScanner::get_interface(&interface_name)?;

    let success = updated.mode == target;
    let message = if success {
        format!("Set {} to {} mode.", interface_name, target)
    } else {
        format!(
            "Command completed but {} reports '{}' instead of '{}'.",
            interface_name, updated.mode, target
        )
    };

    Ok(InterfaceModeResult {
        interface_name,
        previous_mode: current.mode,
        current_mode: updated.mode,
        success,
        message,
    })
}

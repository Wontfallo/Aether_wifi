//! Tauri IPC commands for utility functions:
//! MAC spoofing, SSID list management, WiFi join, AP management.

use crate::network::{mac_spoof, ssid_manager, MacSpoofResult, SsidList};
use crate::network::ap_manager::{self, ApDetail, ApStore, SavedAp};
use tauri::AppHandle;

/// Spoof the MAC address of a wireless interface.
/// If `new_mac` is empty/null, a random MAC is generated.
#[tauri::command]
pub async fn spoof_mac(
    interface_name: String,
    new_mac: Option<String>,
) -> Result<MacSpoofResult, String> {
    let mac_ref = new_mac.as_deref().filter(|s| !s.is_empty());
    mac_spoof::spoof_mac(&interface_name, mac_ref).map_err(|e| e.to_string())
}

/// Restore the original (permanent) MAC address.
#[tauri::command]
pub async fn restore_mac(interface_name: String) -> Result<MacSpoofResult, String> {
    mac_spoof::restore_mac(&interface_name).map_err(|e| e.to_string())
}

/// List all saved SSID list names.
#[tauri::command]
pub async fn list_ssid_lists(app: AppHandle) -> Result<Vec<String>, String> {
    let data_dir = data_dir_for(&app)?;
    Ok(ssid_manager::list_names(&data_dir))
}

/// Get a specific SSID list by name.
#[tauri::command]
pub async fn get_ssid_list(app: AppHandle, name: String) -> Result<Option<SsidList>, String> {
    let data_dir = data_dir_for(&app)?;
    Ok(ssid_manager::get_list(&data_dir, &name))
}

/// Save/create an SSID list.
#[tauri::command]
pub async fn save_ssid_list(
    app: AppHandle,
    name: String,
    ssids: Vec<String>,
) -> Result<(), String> {
    let data_dir = data_dir_for(&app)?;
    ssid_manager::save_list(&data_dir, &name, ssids).map_err(|e| e.to_string())
}

/// Delete an SSID list.
#[tauri::command]
pub async fn delete_ssid_list(app: AppHandle, name: String) -> Result<bool, String> {
    let data_dir = data_dir_for(&app)?;
    ssid_manager::delete_list(&data_dir, &name).map_err(|e| e.to_string())
}

/// Add SSIDs to an existing list (or create it).
#[tauri::command]
pub async fn add_ssids_to_list(
    app: AppHandle,
    name: String,
    ssids: Vec<String>,
) -> Result<usize, String> {
    let data_dir = data_dir_for(&app)?;
    ssid_manager::add_ssids(&data_dir, &name, ssids).map_err(|e| e.to_string())
}

/// Generate random SSIDs.
#[tauri::command]
pub async fn generate_random_ssids(count: usize, max_len: usize) -> Result<Vec<String>, String> {
    Ok(ssid_manager::generate_random_ssids(count, max_len))
}

/// Join a WiFi network using nmcli.
#[tauri::command]
pub async fn join_wifi(
    interface_name: String,
    ssid: String,
    password: Option<String>,
) -> Result<String, String> {
    use std::process::Command;

    // Ensure interface is in managed mode first
    let _ = Command::new("ip")
        .args(["link", "set", &interface_name, "up"])
        .output();

    let output = if let Some(pw) = password.filter(|p| !p.is_empty()) {
        Command::new("nmcli")
            .args([
                "device", "wifi", "connect", &ssid,
                "password", &pw,
                "ifname", &interface_name,
            ])
            .output()
    } else {
        Command::new("nmcli")
            .args([
                "device", "wifi", "connect", &ssid,
                "ifname", &interface_name,
            ])
            .output()
    };

    let output = output.map_err(|e| format!("Failed to run nmcli: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!("nmcli failed: {} {}", stdout, stderr))
    }
}

/// Disconnect from a WiFi network.
#[tauri::command]
pub async fn disconnect_wifi(interface_name: String) -> Result<String, String> {
    use std::process::Command;

    let output = Command::new("nmcli")
        .args(["device", "disconnect", &interface_name])
        .output()
        .map_err(|e| format!("Failed to run nmcli: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(format!(
            "Disconnect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn data_dir_for(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve data dir: {}", e))
}

// ─────────────────────────────────────────────────
// AP List Management
// ─────────────────────────────────────────────────

/// Load all saved APs.
#[tauri::command]
pub async fn load_saved_aps(app: AppHandle) -> Result<Vec<SavedAp>, String> {
    let data_dir = data_dir_for(&app)?;
    Ok(ap_manager::load_aps(&data_dir).aps)
}

/// Save/upsert APs from scan results.
#[tauri::command]
pub async fn save_scan_aps(app: AppHandle, aps: Vec<SavedAp>) -> Result<usize, String> {
    let data_dir = data_dir_for(&app)?;
    ap_manager::upsert_aps(&data_dir, aps).map_err(|e| e.to_string())
}

/// Select APs for targeting.
#[tauri::command]
pub async fn select_target_aps(
    app: AppHandle,
    bssids: Vec<String>,
    selected: bool,
) -> Result<usize, String> {
    let data_dir = data_dir_for(&app)?;
    ap_manager::select_aps(&data_dir, &bssids, selected).map_err(|e| e.to_string())
}

/// Get all selected (targeted) APs.
#[tauri::command]
pub async fn get_selected_aps(app: AppHandle) -> Result<Vec<SavedAp>, String> {
    let data_dir = data_dir_for(&app)?;
    Ok(ap_manager::get_selected_aps(&data_dir))
}

/// Clear all saved APs.
#[tauri::command]
pub async fn clear_saved_aps(app: AppHandle) -> Result<(), String> {
    let data_dir = data_dir_for(&app)?;
    ap_manager::clear_aps(&data_dir).map_err(|e| e.to_string())
}

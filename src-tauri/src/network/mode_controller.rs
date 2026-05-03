//! Mode controller — toggles wireless interfaces between Managed and Monitor mode.
//!
//! Toggle sequence (standard Linux wireless stack):
//!   1. Kill interfering processes  (`airmon-ng check kill` or manual)
//!   2. Bring interface down        (`ip link set <iface> down`)
//!   3. Set the new mode            (`iw <iface> set type <managed|monitor>`)
//!   4. Bring interface back up     (`ip link set <iface> up`)
//!
//! All commands require root privileges. The module detects permission errors
//! and surfaces them as `AetherError::PermissionDenied`.

use log::{debug, info};
use std::process::Command;

use crate::error::{AetherError, AetherResult};
use super::interface_scanner::InterfaceScanner;
use super::types::{InterfaceMode, InterfaceModeResult};

/// Handles toggling wireless interface modes.
pub struct ModeController;

impl ModeController {
    // ─────────────────────────────────────────────
    // Public API
    // ─────────────────────────────────────────────

    /// Toggle an interface to the opposite mode.
    ///
    /// - If currently Managed → switch to Monitor.
    /// - If currently Monitor → switch to Managed.
    /// - If Unknown → attempt switch to Monitor.
    pub fn toggle_mode(interface_name: &str) -> AetherResult<InterfaceModeResult> {
        #[cfg(not(target_os = "linux"))]
        {
            return Err(AetherError::UnsupportedPlatform(
                "Mode toggling is only supported on Linux.".into(),
            ));
        }

        #[cfg(target_os = "linux")]
        {
            let iface = InterfaceScanner::get_interface(interface_name)?;

            if !iface.is_wireless {
                return Err(AetherError::CommandFailed {
                    command: format!("toggle_mode({})", interface_name),
                    detail: format!(
                        "'{}' is not a wireless interface. Mode toggling only applies to WiFi adapters.",
                        interface_name
                    ),
                });
            }

            let target_mode = iface.mode.toggled();

            info!(
                "Toggling {} from {} → {}",
                interface_name, iface.mode, target_mode
            );

            Self::set_mode(interface_name, &target_mode)?;

            // Verify the switch actually worked
            let updated = InterfaceScanner::get_interface(interface_name)?;

            let success = updated.mode == target_mode;
            let message = if success {
                format!(
                    "Successfully switched {} to {} mode.",
                    interface_name, target_mode
                )
            } else {
                format!(
                    "Mode switch command completed but verification shows '{}' instead of '{}'. \
                     The adapter or driver may not support {} mode.",
                    updated.mode, target_mode, target_mode
                )
            };

            Ok(InterfaceModeResult {
                interface_name: interface_name.to_string(),
                previous_mode: iface.mode,
                current_mode: updated.mode,
                success,
                message,
            })
        }
    }

    /// Explicitly set an interface to a specific mode (Managed or Monitor).
    pub fn set_mode(interface_name: &str, target: &InterfaceMode) -> AetherResult<()> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (interface_name, target);
            return Err(AetherError::UnsupportedPlatform(
                "Mode setting is only supported on Linux.".into(),
            ));
        }

        #[cfg(target_os = "linux")]
        {
            if *target == InterfaceMode::Monitor {
                // Step 0: Kill interfering processes (NetworkManager, wpa_supplicant)
                Self::kill_interfering_processes(interface_name);
            }

            // Step 1: Bring the interface down
            info!("Bringing {} down...", interface_name);
            Self::run_privileged("ip", &["link", "set", interface_name, "down"])?;

            // Step 2: Set the mode via `iw`
            let iw_type = target.as_iw_type();
            info!("Setting {} type to {}...", interface_name, iw_type);
            Self::run_privileged("iw", &[interface_name, "set", "type", iw_type])?;

            // Step 3: Bring the interface back up
            info!("Bringing {} up...", interface_name);
            Self::run_privileged("ip", &["link", "set", interface_name, "up"])?;

            if *target == InterfaceMode::Managed {
                Self::restore_managed_services(interface_name);
            }

            info!("Mode change complete: {} → {}", interface_name, target);
            Ok(())
        }
    }

    // ─────────────────────────────────────────────
    // Internal helpers
    // ─────────────────────────────────────────────

    /// Release only the target interface from managed networking before
    /// switching it to monitor mode.
    ///
    /// This intentionally avoids stopping NetworkManager or wpa_supplicant
    /// globally, because doing so can disconnect unrelated interfaces like
    /// Ethernet on the host.
    #[cfg(target_os = "linux")]
    fn kill_interfering_processes(interface_name: &str) {
        info!(
            "Releasing {} from managed networking without touching other interfaces...",
            interface_name
        );

        match Self::run_privileged("nmcli", &["device", "disconnect", interface_name]) {
            Ok(output) => debug!("nmcli device disconnect {}: {}", interface_name, output.trim()),
            Err(e) => debug!(
                "Could not disconnect {} via NetworkManager (may already be unmanaged): {}",
                interface_name, e
            ),
        }

        match Self::run_privileged("nmcli", &["device", "set", interface_name, "managed", "no"]) {
            Ok(output) => debug!("nmcli device set {} managed no: {}", interface_name, output.trim()),
            Err(e) => debug!(
                "Could not mark {} unmanaged via NetworkManager: {}",
                interface_name, e
            ),
        }
    }

    /// Hand the interface back to managed networking after leaving monitor mode.
    ///
    /// Non-fatal — cleanup should not mask a successful mode switch.
    #[cfg(target_os = "linux")]
    fn restore_managed_services(interface_name: &str) {
        info!("Restoring managed networking for {}...", interface_name);

        if let Err(e) = Self::run_privileged(
            "nmcli",
            &["device", "set", interface_name, "managed", "yes"],
        ) {
            debug!(
                "Could not hand {} back to NetworkManager via nmcli: {}",
                interface_name, e
            );
        }

        match Self::run_privileged("nmcli", &["device", "connect", interface_name]) {
            Ok(output) => debug!("nmcli device connect {}: {}", interface_name, output.trim()),
            Err(e) => debug!(
                "Could not reconnect {} via NetworkManager (may require manual reconnect): {}",
                interface_name, e
            ),
        }
    }

    /// Execute a command with sudo/pkexec elevation.
    ///
    /// Tries the command directly first (in case we're already root),
    /// then falls back to `sudo`.
    #[cfg(target_os = "linux")]
    fn run_privileged(program: &str, args: &[&str]) -> AetherResult<String> {
        let cmd_str = format!("{} {}", program, args.join(" "));
        debug!("exec (privileged): {}", cmd_str);

        // Attempt direct execution first (works if running as root)
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| AetherError::CommandFailed {
                command: cmd_str.clone(),
                detail: e.to_string(),
            })?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // If permission denied, retry with sudo
        if stderr.contains("Operation not permitted")
            || stderr.contains("Permission denied")
            || stderr.contains("Not permitted")
        {
            debug!("Permission denied, retrying with sudo...");
            return Self::run_with_sudo(program, args);
        }

        Err(AetherError::CommandFailed {
            command: cmd_str,
            detail: stderr,
        })
    }

    /// Execute a command prefixed with `sudo -n` (non-interactive).
    ///
    /// The `-n` flag prevents sudo from prompting for a password,
    /// which would hang a headless daemon. If passwordless sudo
    /// isn't configured, this returns PermissionDenied.
    #[cfg(target_os = "linux")]
    fn run_with_sudo(program: &str, args: &[&str]) -> AetherResult<String> {
        let mut sudo_args = vec!["-n", program];
        sudo_args.extend_from_slice(args);

        let cmd_str = format!("sudo {} {}", program, args.join(" "));
        debug!("exec: {}", cmd_str);

        let output = Command::new("sudo")
            .args(&sudo_args)
            .output()
            .map_err(|e| AetherError::CommandFailed {
                command: cmd_str.clone(),
                detail: e.to_string(),
            })?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if stderr.contains("a password is required") || stderr.contains("sudo:") {
            return Err(AetherError::PermissionDenied(format!(
                "Passwordless sudo is not configured for '{}'. \
                 Configure sudoers or run Aether-Core as root.",
                program
            )));
        }

        Err(AetherError::CommandFailed {
            command: cmd_str,
            detail: stderr,
        })
    }
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_toggle_logic() {
        // Verify the toggle produces the expected target
        assert_eq!(InterfaceMode::Managed.toggled(), InterfaceMode::Monitor);
        assert_eq!(InterfaceMode::Monitor.toggled(), InterfaceMode::Managed);
        assert_eq!(InterfaceMode::Unknown.toggled(), InterfaceMode::Monitor);
    }

    #[test]
    fn iw_type_strings() {
        assert_eq!(InterfaceMode::Managed.as_iw_type(), "managed");
        assert_eq!(InterfaceMode::Monitor.as_iw_type(), "monitor");
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn toggle_returns_unsupported_on_non_linux() {
        let result = ModeController::toggle_mode("wlan0");
        assert!(result.is_err());
        match result.unwrap_err() {
            AetherError::UnsupportedPlatform(_) => {} // expected
            other => panic!("Expected UnsupportedPlatform, got: {:?}", other),
        }
    }
}

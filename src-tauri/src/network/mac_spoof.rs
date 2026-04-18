//! MAC address spoofing utility.
//!
//! Uses `macchanger` or raw `ip link` to change the MAC address
//! of a wireless interface.

use log::{info, warn};
use std::process::Command;

use super::oui::lookup_vendor;

/// Result of a MAC spoof operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MacSpoofResult {
    pub interface_name: String,
    pub original_mac: String,
    pub new_mac: String,
    pub vendor: Option<String>,
    pub success: bool,
    pub message: String,
}

/// Spoof the MAC address of an interface.
///
/// If `new_mac` is None, generates a random MAC.
/// The interface must be DOWN before changing MAC.
pub fn spoof_mac(
    interface_name: &str,
    new_mac: Option<&str>,
) -> Result<MacSpoofResult, crate::error::AetherError> {
    let original_mac = get_current_mac(interface_name)?;

    // Bring interface down
    run_cmd("ip", &["link", "set", interface_name, "down"])?;

    let result = if command_exists("macchanger") {
        spoof_with_macchanger(interface_name, new_mac)
    } else {
        spoof_with_ip_link(interface_name, new_mac)
    };

    // Bring interface back up regardless
    let _ = run_cmd("ip", &["link", "set", interface_name, "up"]);

    match result {
        Ok(new) => {
            info!("MAC spoofed on '{}': {} → {}", interface_name, original_mac, new);
            Ok(MacSpoofResult {
                interface_name: interface_name.to_string(),
                original_mac,
                new_mac: new.clone(),
                vendor: lookup_vendor(&new).map(|s| s.to_string()),
                success: true,
                message: "MAC address changed successfully.".into(),
            })
        }
        Err(e) => {
            warn!("MAC spoof failed on '{}': {}", interface_name, e);
            // Try to restore original
            let _ = run_cmd("ip", &["link", "set", interface_name, "down"]);
            let _ = run_cmd("ip", &["link", "set", "dev", interface_name, "address", &original_mac]);
            let _ = run_cmd("ip", &["link", "set", interface_name, "up"]);
            Err(e)
        }
    }
}

/// Restore the original (permanent) MAC address.
pub fn restore_mac(
    interface_name: &str,
) -> Result<MacSpoofResult, crate::error::AetherError> {
    let current_mac = get_current_mac(interface_name)?;

    run_cmd("ip", &["link", "set", interface_name, "down"])?;

    let original = if command_exists("macchanger") {
        let output = Command::new("macchanger")
            .arg("-p")
            .arg(interface_name)
            .output()
            .map_err(|e| crate::error::AetherError::CaptureError(format!(
                "Failed to run macchanger: {}", e
            )))?;

        parse_macchanger_output(&String::from_utf8_lossy(&output.stdout))
            .unwrap_or_else(|| get_permanent_mac(interface_name).unwrap_or(current_mac.clone()))
    } else {
        let perm = get_permanent_mac(interface_name)?;
        run_cmd("ip", &["link", "set", "dev", interface_name, "address", &perm])?;
        perm
    };

    let _ = run_cmd("ip", &["link", "set", interface_name, "up"]);

    Ok(MacSpoofResult {
        interface_name: interface_name.to_string(),
        original_mac: current_mac,
        new_mac: original.clone(),
        vendor: lookup_vendor(&original).map(|s| s.to_string()),
        success: true,
        message: "MAC address restored to permanent hardware address.".into(),
    })
}

fn spoof_with_macchanger(
    interface_name: &str,
    new_mac: Option<&str>,
) -> Result<String, crate::error::AetherError> {
    let output = if let Some(mac) = new_mac {
        Command::new("macchanger")
            .arg("-m").arg(mac)
            .arg(interface_name)
            .output()
    } else {
        Command::new("macchanger")
            .arg("-r")
            .arg(interface_name)
            .output()
    };

    let output = output.map_err(|e| crate::error::AetherError::CaptureError(format!(
        "Failed to run macchanger: {}", e
    )))?;

    if !output.status.success() {
        return Err(crate::error::AetherError::CaptureError(format!(
            "macchanger failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_macchanger_output(&stdout).ok_or_else(|| {
        crate::error::AetherError::CaptureError("Could not parse macchanger output".into())
    })
}

fn spoof_with_ip_link(
    interface_name: &str,
    new_mac: Option<&str>,
) -> Result<String, crate::error::AetherError> {
    let mac = match new_mac {
        Some(m) => m.to_string(),
        None => generate_random_mac(),
    };

    run_cmd("ip", &["link", "set", "dev", interface_name, "address", &mac])?;
    Ok(mac)
}

fn get_current_mac(interface_name: &str) -> Result<String, crate::error::AetherError> {
    let path = format!("/sys/class/net/{}/address", interface_name);
    std::fs::read_to_string(&path)
        .map(|s| s.trim().to_ascii_uppercase())
        .map_err(|e| crate::error::AetherError::CaptureError(format!(
            "Cannot read MAC for '{}': {}", interface_name, e
        )))
}

fn get_permanent_mac(interface_name: &str) -> Result<String, crate::error::AetherError> {
    let output = Command::new("ethtool")
        .arg("-P")
        .arg(interface_name)
        .output()
        .map_err(|e| crate::error::AetherError::CaptureError(format!(
            "Failed to run ethtool: {}", e
        )))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output: "Permanent address: aa:bb:cc:dd:ee:ff"
    stdout
        .split(':')
        .skip(1)
        .collect::<Vec<_>>()
        .join(":")
        .trim()
        .to_ascii_uppercase()
        .parse::<String>()
        .ok()
        .filter(|s| s.len() == 17)
        .ok_or_else(|| crate::error::AetherError::CaptureError(
            "Could not determine permanent MAC address".into()
        ))
}

fn parse_macchanger_output(output: &str) -> Option<String> {
    // macchanger output: "New MAC:   xx:xx:xx:xx:xx:xx (vendor)"
    // Find the last line containing "New MAC" or fall back to "Current MAC"
    let target_line = output
        .lines()
        .rev()
        .find(|l| l.contains("New MAC"))
        .or_else(|| output.lines().rev().find(|l| l.contains("Current MAC")))?;

    // Find a MAC address pattern (xx:xx:xx:xx:xx:xx) in the line
    for word in target_line.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_ascii_hexdigit() && c != ':');
        if clean.len() == 17
            && clean.chars().filter(|c| *c == ':').count() == 5
            && clean.split(':').all(|oct| oct.len() == 2 && oct.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Some(clean.to_ascii_uppercase());
        }
    }
    None
}

fn generate_random_mac() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    // Set locally administered bit, clear multicast bit
    let b0 = ((seed & 0xFF) as u8 & 0xFE) | 0x02;
    let b1 = ((seed >> 8) & 0xFF) as u8;
    let b2 = ((seed >> 16) & 0xFF) as u8;
    let b3 = ((seed >> 24) & 0xFF) as u8;
    let b4 = ((seed >> 32) & 0xFF) as u8;
    let b5 = ((seed >> 40) & 0xFF) as u8;

    format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", b0, b1, b2, b3, b4, b5)
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<(), crate::error::AetherError> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| crate::error::AetherError::CaptureError(format!(
            "Failed to run {} {}: {}", cmd, args.join(" "), e
        )))?;

    if !output.status.success() {
        return Err(crate::error::AetherError::CaptureError(format!(
            "{} {} failed: {}",
            cmd,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn command_exists(cmd: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).any(|entry| std::path::Path::new(&entry).join(cmd).exists())
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_macchanger_new_mac() {
        let output = "Current MAC:   00:11:22:33:44:55 (unknown)\nNew MAC:       AA:BB:CC:DD:EE:FF (unknown)\n";
        assert_eq!(
            parse_macchanger_output(output),
            Some("AA:BB:CC:DD:EE:FF".to_string())
        );
    }

    #[test]
    fn generate_random_mac_format() {
        let mac = generate_random_mac();
        assert_eq!(mac.len(), 17);
        assert!(mac.chars().nth(2) == Some(':'));
        // Locally administered bit should be set
        let first_byte = u8::from_str_radix(&mac[0..2], 16).unwrap();
        assert!(first_byte & 0x02 != 0, "Locally administered bit not set");
        assert!(first_byte & 0x01 == 0, "Multicast bit should be clear");
    }

    #[test]
    fn parse_macchanger_current_mac() {
        let output = "Current MAC:   DE:AD:BE:EF:00:01 (vendor info)\n";
        assert_eq!(
            parse_macchanger_output(output),
            Some("DE:AD:BE:EF:00:01".to_string())
        );
    }
}

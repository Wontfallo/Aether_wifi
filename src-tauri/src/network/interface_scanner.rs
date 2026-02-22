//! Interface scanner — discovers network interfaces via Linux system commands.
//!
//! Strategy:
//!   1. Primary:  `iw dev` — gives wireless-specific data (phy, mode, channel).
//!   2. Fallback: `ip link show` — works on all Linux systems, lists all NICs.
//!   3. Enrichment: `iwconfig` — adds driver/chipset info where available.
//!
//! All parsing is done with regex; no heavy C bindings required.

use log::{debug, info, warn};
use regex::Regex;
use std::process::Command;

use crate::error::{AetherError, AetherResult};
use super::types::{InterfaceMode, NetworkInterface};

/// Encapsulates interface discovery logic.
/// Stateless — all methods are associated functions.
pub struct InterfaceScanner;

impl InterfaceScanner {
    // ─────────────────────────────────────────────
    // Public API
    // ─────────────────────────────────────────────

    /// Discover all network interfaces on the system.
    /// Tries `iw dev` first for wireless detail, then falls back to `ip link`.
    pub fn list_interfaces() -> AetherResult<Vec<NetworkInterface>> {
        #[cfg(not(target_os = "linux"))]
        {
            return Err(AetherError::UnsupportedPlatform(
                "Interface scanning is only supported on Linux.".into(),
            ));
        }

        #[cfg(target_os = "linux")]
        {
            info!("Scanning network interfaces...");

            // Try wireless-specific scan first
            let mut interfaces = Self::scan_iw_dev().unwrap_or_else(|e| {
                warn!("iw dev scan failed ({e}), falling back to ip link");
                Vec::new()
            });

            // Merge with ip link data to catch non-wireless & fill gaps
            let ip_link_interfaces = Self::scan_ip_link().unwrap_or_else(|e| {
                warn!("ip link scan failed: {e}");
                Vec::new()
            });

            Self::merge_interfaces(&mut interfaces, ip_link_interfaces);

            // Enrich with iwconfig data (driver, chipset)
            Self::enrich_with_iwconfig(&mut interfaces);

            info!("Discovered {} interface(s)", interfaces.len());
            Ok(interfaces)
        }
    }

    /// Get details for a single interface by name.
    pub fn get_interface(name: &str) -> AetherResult<NetworkInterface> {
        let all = Self::list_interfaces()?;
        all.into_iter()
            .find(|iface| iface.name == name)
            .ok_or_else(|| AetherError::InterfaceNotFound(name.to_string()))
    }

    /// List only wireless interfaces.
    pub fn list_wireless() -> AetherResult<Vec<NetworkInterface>> {
        Ok(Self::list_interfaces()?
            .into_iter()
            .filter(|iface| iface.is_wireless)
            .collect())
    }

    // ─────────────────────────────────────────────
    // Primary scanner: `iw dev`
    // ─────────────────────────────────────────────

    #[cfg(target_os = "linux")]
    fn scan_iw_dev() -> AetherResult<Vec<NetworkInterface>> {
        let output = Self::run_command("iw", &["dev"])?;
        Self::parse_iw_dev(&output)
    }

    /// Parse output of `iw dev` into a list of interfaces.
    ///
    /// Example output:
    /// ```text
    /// phy#0
    ///     Interface wlan0
    ///         ifindex 3
    ///         wdev 0x1
    ///         addr aa:bb:cc:dd:ee:ff
    ///         type managed
    ///         channel 6 (2437 MHz), width: 20 MHz, center1: 2437 MHz
    /// ```
    #[cfg(target_os = "linux")]
    fn parse_iw_dev(output: &str) -> AetherResult<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();
        let mut current_phy: Option<String> = None;
        let mut current_iface: Option<NetworkInterface> = None;

        for line in output.lines() {
            let trimmed = line.trim();

            // New PHY block
            if let Some(phy) = trimmed.strip_prefix("phy#") {
                current_phy = Some(format!("phy{phy}"));

                // Push previous interface if exists
                if let Some(iface) = current_iface.take() {
                    interfaces.push(iface);
                }
                continue;
            }

            // New interface within a PHY
            if let Some(name) = trimmed.strip_prefix("Interface ") {
                // Push previous interface if exists
                if let Some(iface) = current_iface.take() {
                    interfaces.push(iface);
                }

                current_iface = Some(NetworkInterface {
                    name: name.trim().to_string(),
                    mac_address: None,
                    mode: InterfaceMode::Unknown,
                    is_up: false, // Will be determined by ip link
                    is_wireless: true,
                    driver: None,
                    chipset: None,
                    phy: current_phy.clone(),
                });
                continue;
            }

            // Parse fields within an interface block
            if let Some(ref mut iface) = current_iface {
                if let Some(addr) = trimmed.strip_prefix("addr ") {
                    iface.mac_address = Some(addr.trim().to_string());
                } else if let Some(iw_type) = trimmed.strip_prefix("type ") {
                    iface.mode = InterfaceMode::from_str_lossy(iw_type);
                }
            }
        }

        // Don't forget the last one
        if let Some(iface) = current_iface {
            interfaces.push(iface);
        }

        debug!("iw dev: parsed {} wireless interface(s)", interfaces.len());
        Ok(interfaces)
    }

    // ─────────────────────────────────────────────
    // Fallback scanner: `ip link show`
    // ─────────────────────────────────────────────

    #[cfg(target_os = "linux")]
    fn scan_ip_link() -> AetherResult<Vec<NetworkInterface>> {
        let output = Self::run_command("ip", &["-o", "link", "show"])?;
        Self::parse_ip_link(&output)
    }

    /// Parse the one-line-per-interface output of `ip -o link show`.
    ///
    /// Example line:
    /// ```text
    /// 3: wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 ... link/ether aa:bb:cc:dd:ee:ff ...
    /// ```
    #[cfg(target_os = "linux")]
    fn parse_ip_link(output: &str) -> AetherResult<Vec<NetworkInterface>> {
        let re = Regex::new(
            r"^\d+:\s+(\S+?)(?:@\S+)?:\s+<([^>]*)>.*?link/(\S+)\s+([0-9a-f:]{17})?"
        )
        .map_err(|e| AetherError::ParseError {
            origin: "regex".into(),
            detail: e.to_string(),
        })?;

        let mut interfaces = Vec::new();

        for line in output.lines() {
            if let Some(caps) = re.captures(line) {
                let name = caps[1].to_string();
                let flags = &caps[2];
                let link_type = &caps[3];
                let mac = caps.get(4).map(|m| m.as_str().to_string());

                // Skip loopback and virtual interfaces
                if name == "lo" || link_type == "loopback" {
                    continue;
                }

                let is_up = flags.contains("UP");
                let is_wireless = name.starts_with("wl")
                    || name.starts_with("wlan")
                    || name.starts_with("ath")
                    || link_type == "ether"; // will be overridden by iw merge

                interfaces.push(NetworkInterface {
                    name,
                    mac_address: mac,
                    mode: InterfaceMode::Unknown,
                    is_up,
                    is_wireless, // best-guess, corrected by merge
                    driver: None,
                    chipset: None,
                    phy: None,
                });
            }
        }

        debug!("ip link: parsed {} interface(s)", interfaces.len());
        Ok(interfaces)
    }

    // ─────────────────────────────────────────────
    // Enrichment: iwconfig
    // ─────────────────────────────────────────────

    /// Enriches existing interfaces with data from `iwconfig`.
    /// Also marks which interfaces are truly wireless.
    #[cfg(target_os = "linux")]
    fn enrich_with_iwconfig(interfaces: &mut [NetworkInterface]) {
        let output = match Self::run_command("iwconfig", &[]) {
            Ok(o) => o,
            Err(e) => {
                warn!("iwconfig not available ({e}), skipping enrichment");
                return;
            }
        };

        // iwconfig outputs blocks like:
        // wlan0     IEEE 802.11  ESSID:"MyWiFi"
        //           Mode:Managed  Frequency:2.437 GHz  Access Point: ...
        let block_re = Regex::new(r"(?m)^(\S+)\s+IEEE 802\.11").unwrap();
        let mode_re = Regex::new(r"Mode:(\S+)").unwrap();

        let wireless_names: Vec<String> = block_re
            .captures_iter(&output)
            .map(|c| c[1].to_string())
            .collect();

        for iface in interfaces.iter_mut() {
            if wireless_names.contains(&iface.name) {
                iface.is_wireless = true;

                // Extract mode if still unknown
                if iface.mode == InterfaceMode::Unknown {
                    // Find the block for this interface
                    if let Some(pos) = output.find(&iface.name) {
                        let block = &output[pos..];
                        let block_end = block[1..]
                            .find(|c: char| c == '\n' && !c.is_whitespace())
                            .unwrap_or(block.len());
                        let block_text = &block[..block_end];

                        if let Some(mode_cap) = mode_re.captures(block_text) {
                            iface.mode =
                                InterfaceMode::from_str_lossy(&format!("Mode:{}", &mode_cap[1]));
                        }
                    }
                }
            } else {
                // If iw also didn't flag it, it's not wireless
                if iface.phy.is_none() {
                    iface.is_wireless = false;
                }
            }
        }
    }

    // ─────────────────────────────────────────────
    // Merge logic
    // ─────────────────────────────────────────────

    /// Merge ip-link interfaces into the iw-dev list.
    /// - Interfaces already in `primary` get their `is_up` flag updated.
    /// - New interfaces from ip-link are appended (as non-wireless).
    #[cfg(target_os = "linux")]
    fn merge_interfaces(primary: &mut Vec<NetworkInterface>, secondary: Vec<NetworkInterface>) {
        for sec in secondary {
            if let Some(existing) = primary.iter_mut().find(|p| p.name == sec.name) {
                // Update fields from ip link that iw doesn't provide
                existing.is_up = sec.is_up;
                if existing.mac_address.is_none() {
                    existing.mac_address = sec.mac_address;
                }
            } else {
                // New interface not seen by `iw dev` — probably wired
                primary.push(NetworkInterface {
                    is_wireless: false,
                    ..sec
                });
            }
        }
    }

    // ─────────────────────────────────────────────
    // Command runner
    // ─────────────────────────────────────────────

    /// Execute a system command and return its stdout.
    /// Combines stdout and stderr for comprehensive error reporting.
    #[cfg(target_os = "linux")]
    fn run_command(program: &str, args: &[&str]) -> AetherResult<String> {
        debug!("exec: {} {}", program, args.join(" "));

        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| AetherError::CommandFailed {
                command: format!("{} {}", program, args.join(" ")),
                detail: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let detail = if stderr.contains("Operation not permitted")
                || stderr.contains("Permission denied")
            {
                return Err(AetherError::PermissionDenied(format!(
                    "{} {}",
                    program,
                    args.join(" ")
                )));
            } else {
                stderr
            };
            return Err(AetherError::CommandFailed {
                command: format!("{} {}", program, args.join(" ")),
                detail,
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    const IW_DEV_SAMPLE: &str = r#"phy#0
	Unnamed/non-netdev interface
		wdev 0x2
		addr aa:bb:cc:dd:ee:ff
		type P2P-device
	Interface wlan0
		ifindex 3
		wdev 0x1
		addr aa:bb:cc:dd:ee:ff
		type managed
		channel 6 (2437 MHz), width: 20 MHz, center1: 2437 MHz
phy#1
	Interface wlan1mon
		ifindex 5
		wdev 0x100000001
		addr 11:22:33:44:55:66
		type monitor
"#;

    #[cfg(target_os = "linux")]
    const IP_LINK_SAMPLE: &str = r#"1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN mode DEFAULT group default qlen 1000\    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00
2: eth0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc fq_codel state UP mode DEFAULT group default qlen 1000\    link/ether de:ad:be:ef:00:01 brd ff:ff:ff:ff:ff:ff
3: wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc noqueue state UP mode DORMANT group default qlen 1000\    link/ether aa:bb:cc:dd:ee:ff brd ff:ff:ff:ff:ff:ff
"#;

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_iw_dev_extracts_interfaces() {
        let interfaces = InterfaceScanner::parse_iw_dev(IW_DEV_SAMPLE).unwrap();

        // Should find wlan0 and wlan1mon (skipping P2P)
        assert_eq!(interfaces.len(), 2);

        let wlan0 = &interfaces[0];
        assert_eq!(wlan0.name, "wlan0");
        assert_eq!(wlan0.mode, InterfaceMode::Managed);
        assert_eq!(wlan0.mac_address.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert_eq!(wlan0.phy.as_deref(), Some("phy0"));
        assert!(wlan0.is_wireless);

        let wlan1mon = &interfaces[1];
        assert_eq!(wlan1mon.name, "wlan1mon");
        assert_eq!(wlan1mon.mode, InterfaceMode::Monitor);
        assert_eq!(wlan1mon.phy.as_deref(), Some("phy1"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_ip_link_extracts_interfaces() {
        let interfaces = InterfaceScanner::parse_ip_link(IP_LINK_SAMPLE).unwrap();

        // lo should be filtered out
        assert!(interfaces.iter().all(|i| i.name != "lo"));

        let eth0 = interfaces.iter().find(|i| i.name == "eth0").unwrap();
        assert!(eth0.is_up);

        let wlan0 = interfaces.iter().find(|i| i.name == "wlan0").unwrap();
        assert!(wlan0.is_up);
    }

    #[test]
    fn interface_mode_toggle() {
        assert_eq!(InterfaceMode::Managed.toggled(), InterfaceMode::Monitor);
        assert_eq!(InterfaceMode::Monitor.toggled(), InterfaceMode::Managed);
    }

    #[test]
    fn interface_mode_from_strings() {
        assert_eq!(InterfaceMode::from_str_lossy("managed"), InterfaceMode::Managed);
        assert_eq!(InterfaceMode::from_str_lossy("monitor"), InterfaceMode::Monitor);
        assert_eq!(InterfaceMode::from_str_lossy("Mode:Managed"), InterfaceMode::Managed);
        assert_eq!(InterfaceMode::from_str_lossy("gibberish"), InterfaceMode::Unknown);
    }
}

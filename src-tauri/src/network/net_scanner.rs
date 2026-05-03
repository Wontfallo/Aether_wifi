//! Network scanner module — wraps nmap & arp-scan for host/port/service discovery.
//!
//! All functions spawn external tools as child processes, parse their
//! grepable (`-oG -`) output, and return structured Rust types.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;

use crate::error::AetherError;
use crate::network::types::{HostInfo, PortResult, ServiceInfo};

fn split_targets(target: &str) -> Vec<String> {
    target
        .split(|character: char| character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Current Unix timestamp in milliseconds.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─────────────────────────────────────────────────
// Ping Scan  (nmap -sn)
// ─────────────────────────────────────────────────

/// Discover live hosts on `subnet` using ICMP/ARP ping (`nmap -sn`).
///
/// Requires root for ARP-level discovery on local subnets.
pub fn ping_scan(subnet: &str) -> Result<Vec<HostInfo>, AetherError> {
    let output = Command::new("nmap")
        .args(["-sn", "-oG", "-", subnet])
        .output()
        .map_err(|e| AetherError::CommandFailed {
            command: "nmap -sn".into(),
            detail: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AetherError::CommandFailed {
            command: "nmap -sn".into(),
            detail: stderr.to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_ping_scan_output(&stdout))
}

/// Parse nmap `-sn -oG -` grepable output into `HostInfo` entries.
pub(crate) fn parse_ping_scan_output(output: &str) -> Vec<HostInfo> {
    let ts = now_ms();
    let mut hosts = Vec::new();

    for line in output.lines() {
        if !line.starts_with("Host:") {
            continue;
        }
        // Format: Host: 192.168.1.1 (hostname)	Status: Up
        // or:     Host: 192.168.1.1 ()	Status: Up
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() < 2 {
            continue;
        }

        let host_part = parts[0]; // "Host: 192.168.1.1 (hostname)"
        let status_part = parts[1]; // "Status: Up"

        let is_up = status_part.contains("Up");

        // Extract IP and hostname from "Host: <ip> (<hostname>)"
        let after_host = host_part.strip_prefix("Host: ").unwrap_or(host_part);
        let (ip, hostname) = if let Some(paren_start) = after_host.find('(') {
            let ip = after_host[..paren_start].trim().to_string();
            let rest = &after_host[paren_start + 1..];
            let hostname = rest.trim_end_matches(')').trim().to_string();
            let hostname = if hostname.is_empty() {
                None
            } else {
                Some(hostname)
            };
            (ip, hostname)
        } else {
            (after_host.trim().to_string(), None)
        };

        hosts.push(HostInfo {
            ip,
            mac: None,
            hostname,
            vendor: None,
            is_up,
            timestamp_ms: ts,
        });
    }
    hosts
}

// ─────────────────────────────────────────────────
// ARP Scan  (arp-scan or nmap -sn -PR)
// ─────────────────────────────────────────────────

/// Discover hosts on the local segment via ARP using `arp-scan`.
///
/// Falls back to `nmap -sn -PR` if arp-scan is not installed.
pub fn arp_scan(interface: &str) -> Result<Vec<HostInfo>, AetherError> {
    // Try arp-scan first
    let arp_result = Command::new("arp-scan")
        .args(["--interface", interface, "--localnet", "--plain"])
        .output();

    match arp_result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(parse_arp_scan_output(&stdout))
        }
        _ => {
            // Fallback: nmap ARP scan
            let output = Command::new("nmap")
                .args(["-sn", "-PR", "-oG", "-", "-e", interface, "--send-eth", "0.0.0.0/0"])
                .output()
                .map_err(|e| AetherError::CommandFailed {
                    command: "nmap -sn -PR".into(),
                    detail: e.to_string(),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AetherError::CommandFailed {
                    command: "nmap -sn -PR".into(),
                    detail: stderr.to_string(),
                });
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(parse_ping_scan_output(&stdout))
        }
    }
}

/// Parse `arp-scan --plain` output.
///
/// Each line: `<IP>\t<MAC>\t<Vendor>`
pub(crate) fn parse_arp_scan_output(output: &str) -> Vec<HostInfo> {
    let ts = now_ms();
    let mut hosts = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Interface:") || line.starts_with("Starting") || line.ends_with("responded") {
            continue;
        }

        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 {
            continue;
        }

        // Validate first column looks like an IP
        if !cols[0].contains('.') {
            continue;
        }

        let ip = cols[0].trim().to_string();
        let mac = Some(cols[1].trim().to_string());
        let vendor = if cols.len() >= 3 && !cols[2].trim().is_empty() {
            Some(cols[2].trim().to_string())
        } else {
            None
        };

        hosts.push(HostInfo {
            ip,
            mac,
            hostname: None,
            vendor,
            is_up: true,
            timestamp_ms: ts,
        });
    }
    hosts
}

// ─────────────────────────────────────────────────
// Port Scan  (nmap -p)
// ─────────────────────────────────────────────────

/// Scan `target` for open/closed/filtered ports specified by `ports`.
///
/// `ports` uses nmap syntax: `"22"`, `"1-1024"`, `"22,80,443"`, etc.
pub fn port_scan(target: &str, ports: &str) -> Result<Vec<PortResult>, AetherError> {
    let mut args = vec![
        "-p".to_string(),
        ports.to_string(),
        "-oG".to_string(),
        "-".to_string(),
    ];
    let targets = split_targets(target);

    if targets.is_empty() {
        return Err(AetherError::CommandFailed {
            command: "nmap -p".into(),
            detail: "No scan target was provided.".into(),
        });
    }

    args.extend(targets);

    let output = Command::new("nmap")
        .args(&args)
        .output()
        .map_err(|e| AetherError::CommandFailed {
            command: "nmap -p".into(),
            detail: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AetherError::CommandFailed {
            command: "nmap -p".into(),
            detail: stderr.to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_port_scan_output(&stdout))
}

/// Parse nmap `-p -oG -` grepable output into `PortResult` entries.
///
/// Grepable port lines look like:
/// `Host: 192.168.1.1 ()	Ports: 22/open/tcp//ssh///, 80/open/tcp//http///`
pub(crate) fn parse_port_scan_output(output: &str) -> Vec<PortResult> {
    let mut results = Vec::new();
    // Match: port/state/protocol//service//version/
    let port_re = Regex::new(r"(\d+)/(open|closed|filtered)/(tcp|udp)//([^/]*)//([^/]*)/").unwrap();

    for line in output.lines() {
        if !line.starts_with("Host:") || !line.contains("Ports:") {
            continue;
        }

        // Extract host IP
        let host = extract_host_ip(line);

        // Extract ports section
        if let Some(ports_start) = line.find("Ports:") {
            let ports_section = &line[ports_start + 6..];
            // Ports may be followed by other tab-separated sections
            let ports_str = ports_section.split('\t').next().unwrap_or(ports_section);

            for cap in port_re.captures_iter(ports_str) {
                let port: u16 = cap[1].parse().unwrap_or(0);
                let state = cap[2].to_string();
                let protocol = cap[3].to_string();
                let service = if cap[4].is_empty() {
                    None
                } else {
                    Some(cap[4].to_string())
                };
                let version = if cap[5].is_empty() {
                    None
                } else {
                    Some(cap[5].to_string())
                };

                results.push(PortResult {
                    host: host.clone(),
                    port,
                    protocol,
                    state,
                    service,
                    version,
                });
            }
        }
    }
    results
}

// ─────────────────────────────────────────────────
// Service Scan helpers  (SSH / Telnet)
// ─────────────────────────────────────────────────

/// Scan for SSH services (`nmap -p22 --open -sV`).
pub fn ssh_scan(subnet: &str) -> Result<Vec<ServiceInfo>, AetherError> {
    service_scan(subnet, "22", "ssh")
}

/// Scan for Telnet services (`nmap -p23 --open -sV`).
pub fn telnet_scan(subnet: &str) -> Result<Vec<ServiceInfo>, AetherError> {
    service_scan(subnet, "23", "telnet")
}

/// Run a preset post-connect service discovery profile against one host,
/// many hosts, or a subnet.
pub fn service_profile_scan(target: &str, profile: &str) -> Result<Vec<ServiceInfo>, AetherError> {
    let targets = split_targets(target);
    if targets.is_empty() {
        return Err(AetherError::CommandFailed {
            command: "nmap service profile".into(),
            detail: "No scan target was provided.".into(),
        });
    }

    let mut args = vec![
        "-n".to_string(),
        "--open".to_string(),
        "-sV".to_string(),
        "--version-light".to_string(),
        "-oG".to_string(),
        "-".to_string(),
    ];

    let label = match profile {
        "quick_tcp" => {
            args.extend(["-sS", "--top-ports", "250"].iter().map(|value| value.to_string()));
            "quick_tcp"
        }
        "web_admin" => {
            args.extend(["-sS", "-p", "80,81,443,591,8000,8080,8081,8443,8888"].iter().map(|value| value.to_string()));
            "web_admin"
        }
        "file_shares" => {
            args.extend(["-sS", "-p", "111,139,445,2049"].iter().map(|value| value.to_string()));
            "file_shares"
        }
        "camera_streams" => {
            args.extend(["-sS", "-p", "80,81,443,554,8000,8080,8554"].iter().map(|value| value.to_string()));
            "camera_streams"
        }
        "udp_infra" => {
            args.extend(["-sU", "-p", "53,67,68,69,123,137,161,1900,5353"].iter().map(|value| value.to_string()));
            "udp_infra"
        }
        _ => {
            return Err(AetherError::CommandFailed {
                command: "nmap service profile".into(),
                detail: format!("Unknown service discovery profile '{}'.", profile),
            });
        }
    };

    args.extend(targets);

    let output = Command::new("nmap")
        .args(&args)
        .output()
        .map_err(|e| AetherError::CommandFailed {
            command: format!("nmap service profile ({})", label),
            detail: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AetherError::CommandFailed {
            command: format!("nmap service profile ({})", label),
            detail: stderr.to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_service_scan_output(&stdout))
}

/// Generic service scan: runs `nmap -p<port> --open -sV -oG -` on `subnet`.
fn service_scan(subnet: &str, port: &str, label: &str) -> Result<Vec<ServiceInfo>, AetherError> {
    let mut args = vec![
        "-p".to_string(),
        port.to_string(),
        "--open".to_string(),
        "-sV".to_string(),
        "-oG".to_string(),
        "-".to_string(),
    ];
    args.extend(split_targets(subnet));

    let output = Command::new("nmap")
        .args(&args)
        .output()
        .map_err(|e| AetherError::CommandFailed {
            command: format!("nmap -p{} -sV ({})", port, label),
            detail: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AetherError::CommandFailed {
            command: format!("nmap -p{} -sV ({})", port, label),
            detail: stderr.to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_service_scan_output(&stdout))
}

/// Parse nmap `-sV -oG -` grepable output into `ServiceInfo` entries.
pub(crate) fn parse_service_scan_output(output: &str) -> Vec<ServiceInfo> {
    let mut results = Vec::new();
    let port_re = Regex::new(r"(\d+)/open/(tcp|udp)//([^/]*)//([^/]*)/").unwrap();

    for line in output.lines() {
        if !line.starts_with("Host:") || !line.contains("Ports:") {
            continue;
        }

        let host = extract_host_ip(line);

        if let Some(ports_start) = line.find("Ports:") {
            let ports_section = &line[ports_start + 6..];
            let ports_str = ports_section.split('\t').next().unwrap_or(ports_section);

            for cap in port_re.captures_iter(ports_str) {
                let port: u16 = cap[1].parse().unwrap_or(0);
                let service = cap[3].to_string();
                let version = if cap[4].is_empty() {
                    None
                } else {
                    Some(cap[4].to_string())
                };

                results.push(ServiceInfo {
                    host: host.clone(),
                    port,
                    protocol: cap[2].to_string(),
                    service,
                    version,
                    mac: None,
                    vendor: None,
                });
            }
        }
    }
    results
}

/// Extract host IP from a grepable line starting with `Host: <ip> (...)`.
fn extract_host_ip(line: &str) -> String {
    let after = line.strip_prefix("Host: ").unwrap_or(line);
    after
        .split_whitespace()
        .next()
        .unwrap_or("unknown")
        .to_string()
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ping_scan_two_hosts() {
        let output = "\
# Nmap 7.94 scan initiated as: nmap -sn -oG - 192.168.1.0/24
Host: 192.168.1.1 (gateway.local)\tStatus: Up
Host: 192.168.1.42 ()\tStatus: Up
# Nmap done: 256 IP addresses (2 hosts up)
";
        let hosts = parse_ping_scan_output(output);
        assert_eq!(hosts.len(), 2);

        assert_eq!(hosts[0].ip, "192.168.1.1");
        assert_eq!(hosts[0].hostname.as_deref(), Some("gateway.local"));
        assert!(hosts[0].is_up);

        assert_eq!(hosts[1].ip, "192.168.1.42");
        assert!(hosts[1].hostname.is_none());
        assert!(hosts[1].is_up);
    }

    #[test]
    fn parse_ping_scan_empty() {
        let output = "# Nmap done: 256 IP addresses (0 hosts up)\n";
        let hosts = parse_ping_scan_output(output);
        assert!(hosts.is_empty());
    }

    #[test]
    fn parse_ping_scan_down_host() {
        let output = "Host: 10.0.0.5 ()\tStatus: Down\n";
        let hosts = parse_ping_scan_output(output);
        assert_eq!(hosts.len(), 1);
        assert!(!hosts[0].is_up);
    }

    #[test]
    fn parse_arp_scan_basic() {
        let output = "\
192.168.1.1\t00:11:22:33:44:55\tCisco Systems
192.168.1.100\tAA:BB:CC:DD:EE:FF\tApple, Inc.
";
        let hosts = parse_arp_scan_output(output);
        assert_eq!(hosts.len(), 2);

        assert_eq!(hosts[0].ip, "192.168.1.1");
        assert_eq!(hosts[0].mac.as_deref(), Some("00:11:22:33:44:55"));
        assert_eq!(hosts[0].vendor.as_deref(), Some("Cisco Systems"));
        assert!(hosts[0].is_up);

        assert_eq!(hosts[1].ip, "192.168.1.100");
        assert_eq!(hosts[1].mac.as_deref(), Some("AA:BB:CC:DD:EE:FF"));
    }

    #[test]
    fn parse_arp_scan_skips_header_lines() {
        let output = "\
Interface: eth0, type: EN10MB, MAC: 00:11:22:33:44:55
Starting arp-scan 1.10.0
192.168.1.1\t00:11:22:33:44:55\tVendor
3 packets received by filter, 0 packets dropped by kernel
";
        let hosts = parse_arp_scan_output(output);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].ip, "192.168.1.1");
    }

    #[test]
    fn parse_arp_scan_empty() {
        let output = "";
        let hosts = parse_arp_scan_output(output);
        assert!(hosts.is_empty());
    }

    #[test]
    fn parse_port_scan_multiple_ports() {
        let output = "\
# Nmap 7.94 scan initiated as: nmap -p 22,80,443 -oG - 192.168.1.1
Host: 192.168.1.1 ()\tPorts: 22/open/tcp//ssh///, 80/open/tcp//http///, 443/closed/tcp//https///
# Nmap done: 1 IP address (1 host up)
";
        let results = parse_port_scan_output(output);
        assert_eq!(results.len(), 3);

        assert_eq!(results[0].port, 22);
        assert_eq!(results[0].state, "open");
        assert_eq!(results[0].service.as_deref(), Some("ssh"));
        assert_eq!(results[0].host, "192.168.1.1");

        assert_eq!(results[1].port, 80);
        assert_eq!(results[1].state, "open");
        assert_eq!(results[1].service.as_deref(), Some("http"));

        assert_eq!(results[2].port, 443);
        assert_eq!(results[2].state, "closed");
        assert_eq!(results[2].service.as_deref(), Some("https"));
    }

    #[test]
    fn parse_port_scan_no_hosts() {
        let output = "# Nmap done: 0 IP addresses (0 hosts up)\n";
        let results = parse_port_scan_output(output);
        assert!(results.is_empty());
    }

    #[test]
    fn parse_port_scan_filtered_port() {
        let output = "Host: 10.0.0.1 (firewall)\tPorts: 8080/filtered/tcp//http-proxy///\n";
        let results = parse_port_scan_output(output);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].state, "filtered");
        assert_eq!(results[0].port, 8080);
        assert_eq!(results[0].service.as_deref(), Some("http-proxy"));
    }

    #[test]
    fn parse_service_scan_ssh() {
        let output = "\
# Nmap 7.94 scan initiated as: nmap -p 22 --open -sV -oG - 192.168.1.0/24
Host: 192.168.1.1 (gateway)\tPorts: 22/open/tcp//ssh//OpenSSH 8.9p1/
Host: 192.168.1.50 ()\tPorts: 22/open/tcp//ssh//dropbear 2022.83/
# Nmap done: 256 IP addresses (2 hosts up)
";
        let results = parse_service_scan_output(output);
        assert_eq!(results.len(), 2);

        assert_eq!(results[0].host, "192.168.1.1");
        assert_eq!(results[0].port, 22);
        assert_eq!(results[0].service, "ssh");
        assert_eq!(results[0].version.as_deref(), Some("OpenSSH 8.9p1"));

        assert_eq!(results[1].host, "192.168.1.50");
        assert_eq!(results[1].version.as_deref(), Some("dropbear 2022.83"));
    }

    #[test]
    fn parse_service_scan_no_version() {
        let output = "Host: 10.0.0.5 ()\tPorts: 23/open/tcp//telnet///\n";
        let results = parse_service_scan_output(output);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].service, "telnet");
        assert_eq!(results[0].protocol, "tcp");
        assert!(results[0].version.is_none());
    }

    #[test]
    fn parse_service_scan_empty() {
        let output = "# Nmap done: 256 IP addresses (0 hosts up)\n";
        let results = parse_service_scan_output(output);
        assert!(results.is_empty());
    }

    #[test]
    fn parse_service_scan_udp_protocol() {
        let output = "Host: 10.0.0.53 ()\tPorts: 161/open/udp//snmp//SNMPv2 server/\n";
        let results = parse_service_scan_output(output);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].protocol, "udp");
        assert_eq!(results[0].service, "snmp");
        assert_eq!(results[0].version.as_deref(), Some("SNMPv2 server"));
    }

    #[test]
    fn extract_host_ip_with_hostname() {
        assert_eq!(extract_host_ip("Host: 10.0.0.1 (router.local)\tPorts:"), "10.0.0.1");
    }

    #[test]
    fn extract_host_ip_no_hostname() {
        assert_eq!(extract_host_ip("Host: 10.0.0.1 ()\tStatus: Up"), "10.0.0.1");
    }
}

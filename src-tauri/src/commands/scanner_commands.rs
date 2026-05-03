//! Tauri IPC command handlers for network scanning operations.
//!
//! These commands wrap the `net_scanner` module, delegating to
//! nmap/arp-scan for host/port/service discovery.

use crate::error::AetherError;
use crate::network::net_scanner;
use crate::network::types::{HostInfo, PortResult, ServiceInfo};

/// Discover live hosts on a subnet via ping/ARP sweep.
///
/// Invoked from frontend: `invoke('ping_scan', { subnet: '192.168.1.0/24' })`
#[tauri::command]
pub fn ping_scan(subnet: String) -> Result<Vec<HostInfo>, AetherError> {
    log::info!("[cmd] ping_scan: {}", subnet);
    net_scanner::ping_scan(&subnet)
}

/// Discover hosts via ARP on a local network interface.
///
/// Invoked from frontend: `invoke('arp_scan', { interfaceName: 'eth0' })`
#[tauri::command]
pub fn arp_scan(interface_name: String) -> Result<Vec<HostInfo>, AetherError> {
    log::info!("[cmd] arp_scan: {}", interface_name);
    net_scanner::arp_scan(&interface_name)
}

/// Scan specific ports on a target host or subnet.
///
/// Invoked from frontend: `invoke('port_scan', { target: '192.168.1.1', ports: '22,80,443' })`
#[tauri::command]
pub fn port_scan(target: String, ports: String) -> Result<Vec<PortResult>, AetherError> {
    log::info!("[cmd] port_scan: {} ports={}", target, ports);
    net_scanner::port_scan(&target, &ports)
}

/// Scan a subnet for SSH services (port 22).
///
/// Invoked from frontend: `invoke('ssh_scan', { subnet: '192.168.1.0/24' })`
#[tauri::command]
pub fn ssh_scan(subnet: String) -> Result<Vec<ServiceInfo>, AetherError> {
    log::info!("[cmd] ssh_scan: {}", subnet);
    net_scanner::ssh_scan(&subnet)
}

/// Scan a subnet for Telnet services (port 23).
///
/// Invoked from frontend: `invoke('telnet_scan', { subnet: '192.168.1.0/24' })`
#[tauri::command]
pub fn telnet_scan(subnet: String) -> Result<Vec<ServiceInfo>, AetherError> {
    log::info!("[cmd] telnet_scan: {}", subnet);
    net_scanner::telnet_scan(&subnet)
}

/// Run a preset post-connect service discovery sweep against a host list or subnet.
///
/// Invoked from frontend: `invoke('service_profile_scan', { target: '192.168.1.0/24', profile: 'quick_tcp' })`
#[tauri::command]
pub fn service_profile_scan(target: String, profile: String) -> Result<Vec<ServiceInfo>, AetherError> {
    log::info!("[cmd] service_profile_scan: {} profile={}", target, profile);
    net_scanner::service_profile_scan(&target, &profile)
}

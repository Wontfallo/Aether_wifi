//! Aether-Core — the privileged Rust backend for the Aether WiFi analyzer.
//!
//! This crate powers the Tauri desktop application, handling:
//! - Network interface discovery and enumeration
//! - Wireless adapter mode management (Managed <-> Monitor)
//! - Raw 802.11 packet capture (Beacon frame parsing, RSSI extraction)
//! - Deauthentication frame injection and EAPOL handshake capture
//! - Real-time event streaming to the React frontend
//!
//! # Architecture
//!
//! ```text
//! +---------------------------------------------+
//! |  React Frontend (Aether-UI)                 |
//! |  +- invoke('list_interfaces')               |
//! |  +- invoke('start_capture', {...})           |
//! |  +- invoke('send_deauth', {...})             |
//! |  +- invoke('one_click_capture', {...})       |
//! |  +- listen('beacon-frame', callback)         |
//! |  +- listen('eapol-status', callback)         |
//! +---------------------------------------------+
//! |  Tauri IPC Bridge (commands/)               |
//! |  +-- network_commands  (scan, mode toggle)  |
//! |  +-- capture_commands  (start/stop sniffer) |
//! |  +-- audit_commands    (deauth, EAPOL)      |
//! +---------------------------------------------+
//! |  Business Logic (network/)                  |
//! |  +-- InterfaceScanner  (iw dev / ip link)   |
//! |  +-- ModeController    (iw set type)        |
//! |  +-- PacketSniffer     (pcap + 802.11)      |
//! |  +-- Audit             (deauth + EAPOL)     |
//! +---------------------------------------------+
//! |  Linux Kernel (nl80211 / wireless stack)    |
//! +---------------------------------------------+
//! ```

mod commands;
pub mod error;
pub mod network;

use commands::{attack_commands, audit_commands, capture_commands, network_commands, scanner_commands, sniffer_commands, utility_commands};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize env_logger for structured log output in development
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("Aether-Core v{} starting...", env!("CARGO_PKG_VERSION"));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Managed state: tracks the active packet capture session
        .manage(capture_commands::CaptureState(std::sync::Mutex::new(None)))
        // Managed state: tracks the active EAPOL handshake capture
        .manage(audit_commands::EapolCaptureState(std::sync::Arc::new(std::sync::Mutex::new(
            None,
        ))))
        // Managed state: tracks the active deauth attack
        .manage(audit_commands::DeauthState(std::sync::Arc::new(std::sync::Mutex::new(None))))
        // Managed state: tracks the active frame sniffer (probe/deauth detection)
        .manage(sniffer_commands::SnifferState(std::sync::Mutex::new(None)))
        // Managed state: tracks the active mdk4 attack
        .manage(attack_commands::AttackState(std::sync::Mutex::new(None)))
        // Managed state: tracks the bettercap daemon
        .manage(attack_commands::BettercapState(std::sync::Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            // Network interface management
            network_commands::list_interfaces,
            network_commands::list_wireless_interfaces,
            network_commands::get_interface,
            network_commands::get_monitor_interface,
            network_commands::toggle_interface_mode,
            network_commands::set_interface_mode,
            // Packet capture
            capture_commands::start_capture,
            capture_commands::stop_capture,
            capture_commands::capture_status,
            // Offensive audit suite
            audit_commands::start_deauth,
            audit_commands::stop_deauth,
            audit_commands::start_eapol_capture,
            audit_commands::stop_eapol_capture,
            audit_commands::one_click_capture,
            audit_commands::stop_all_attacks,
            // Frame sniffer (probe requests + deauth detection)
            sniffer_commands::start_sniffer,
            sniffer_commands::stop_sniffer,
            // Attack engine (mdk4 + bettercap)
            attack_commands::start_beacon_spam,
            attack_commands::start_probe_flood,
            attack_commands::start_mdk4_deauth,
            attack_commands::stop_attack,
            attack_commands::start_bettercap_daemon,
            attack_commands::stop_bettercap_daemon,
            attack_commands::start_evil_portal,
            attack_commands::start_karma_attack,
            attack_commands::bettercap_command,
            // Utility commands (MAC spoof, SSID manager, WiFi join)
            utility_commands::spoof_mac,
            utility_commands::restore_mac,
            utility_commands::list_ssid_lists,
            utility_commands::get_ssid_list,
            utility_commands::save_ssid_list,
            utility_commands::delete_ssid_list,
            utility_commands::add_ssids_to_list,
            utility_commands::generate_random_ssids,
            utility_commands::join_wifi,
            utility_commands::disconnect_wifi,
            // Network scanner (ping, ARP, port, SSH, telnet)
            scanner_commands::ping_scan,
            scanner_commands::arp_scan,
            scanner_commands::port_scan,
            scanner_commands::ssh_scan,
            scanner_commands::telnet_scan,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aether application");
}

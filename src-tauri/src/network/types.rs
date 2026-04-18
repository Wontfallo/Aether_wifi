//! Data types for the Aether network subsystem.
//!
//! These structs and enums are the core domain models that flow
//! between the Rust backend and the React frontend via Tauri IPC.

use serde::{Deserialize, Serialize};

/// The operating mode of a wireless network interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceMode {
    /// Standard client mode — connects to access points.
    Managed,
    /// Raw packet capture mode — sniffs all 802.11 frames.
    Monitor,
    /// Mode could not be determined.
    Unknown,
}

impl InterfaceMode {
    /// Parse a mode string (from `iw` or `iwconfig` output) into an enum variant.
    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "managed" => InterfaceMode::Managed,
            "monitor" => InterfaceMode::Monitor,
            // `iw` reports "managed" but `iwconfig` uses "Mode:Managed"
            "mode:managed" => InterfaceMode::Managed,
            "mode:monitor" => InterfaceMode::Monitor,
            _ => InterfaceMode::Unknown,
        }
    }

    /// Returns the `iw` type string used in `iw <iface> set type <mode>`.
    pub fn as_iw_type(&self) -> &'static str {
        match self {
            InterfaceMode::Managed => "managed",
            InterfaceMode::Monitor => "monitor",
            InterfaceMode::Unknown => "managed", // default fallback
        }
    }

    /// The opposite mode — used for toggle operations.
    pub fn toggled(&self) -> Self {
        match self {
            InterfaceMode::Managed => InterfaceMode::Monitor,
            InterfaceMode::Monitor => InterfaceMode::Managed,
            InterfaceMode::Unknown => InterfaceMode::Monitor,
        }
    }
}

impl std::fmt::Display for InterfaceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterfaceMode::Managed => write!(f, "Managed"),
            InterfaceMode::Monitor => write!(f, "Monitor"),
            InterfaceMode::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Represents a single network interface discovered on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Interface name (e.g. "wlan0", "wlp3s0").
    pub name: String,

    /// Hardware MAC address (e.g. "aa:bb:cc:dd:ee:ff").
    pub mac_address: Option<String>,

    /// Current operating mode.
    pub mode: InterfaceMode,

    /// Whether the interface is currently UP.
    pub is_up: bool,

    /// Whether this is a wireless interface.
    pub is_wireless: bool,

    /// Kernel driver in use (e.g. "ath9k", "rtl8812au").
    pub driver: Option<String>,

    /// Chipset / device description.
    pub chipset: Option<String>,

    /// PHY index from `iw dev` (e.g. "phy0").
    pub phy: Option<String>,
}

/// Result of a mode toggle operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceModeResult {
    /// The interface that was modified.
    pub interface_name: String,

    /// The mode before the toggle.
    pub previous_mode: InterfaceMode,

    /// The mode after the toggle.
    pub current_mode: InterfaceMode,

    /// Whether the operation succeeded.
    pub success: bool,

    /// Human-readable status message.
    pub message: String,
}

// ─────────────────────────────────────────────────
// Packet Capture Types
// ─────────────────────────────────────────────────

/// A parsed 802.11 Beacon frame — the data streamed to the frontend.
///
/// Emitted via Tauri event `"beacon-frame"` each time the sniffer
/// captures and successfully parses a beacon from an access point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconFrame {
    /// BSSID of the access point (MAC address, e.g. "AA:BB:CC:DD:EE:FF").
    pub bssid: String,

    /// Network name. Empty string for hidden networks.
    pub ssid: String,

    /// WiFi channel number (1–14 for 2.4 GHz, 36–165 for 5 GHz).
    pub channel: u8,

    /// Received Signal Strength Indicator in dBm (e.g. −45).
    /// Typical range: −100 (weak) to −20 (very strong).
    pub rssi: i8,

    /// Frequency in MHz (e.g. 2437 for channel 6).
    pub frequency_mhz: u16,

    /// Vendor name resolved from OUI lookup on the BSSID.
    pub vendor: Option<String>,

    /// Unix timestamp (milliseconds) when this frame was captured.
    pub timestamp_ms: u64,
}

/// A discovered WiFi client station (from airodump-ng station section).
///
/// Emitted via Tauri event `"station-info"` each time the sniffer
/// discovers a client device during an airodump-ng capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationInfo {
    /// Client device MAC address (e.g. "AA:BB:CC:DD:EE:FF").
    pub mac: String,

    /// Associated AP BSSID (`None` if not associated to any AP).
    pub associated_bssid: Option<String>,

    /// Signal strength in dBm (e.g. −65).
    pub rssi: i8,

    /// Number of packets captured from this station.
    pub packet_count: u32,

    /// List of SSIDs this station has probed for.
    pub probed_ssids: Vec<String>,

    /// Vendor name from OUI lookup (filled in later when OUI module is ready).
    pub vendor: Option<String>,

    /// Unix timestamp (milliseconds) when this station was last seen.
    pub timestamp_ms: u64,
}

/// Status payload returned by start/stop capture commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureStatus {
    /// The interface being captured on.
    pub interface_name: String,

    /// Whether a capture is currently active.
    pub active: bool,

    /// Human-readable message.
    pub message: String,
}

// ─────────────────────────────────────────────────
// Sniffer Frame Types
// ─────────────────────────────────────────────────

/// A parsed 802.11 Probe Request frame.
///
/// Emitted via Tauri event `"probe-request"` when the sniffer detects
/// a client device actively scanning for networks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeRequest {
    /// Source MAC of the probing device.
    pub source_mac: String,

    /// SSID being probed for. Empty string = wildcard/broadcast probe.
    pub ssid: String,

    /// Signal strength in dBm.
    pub rssi: i8,

    /// Channel the probe was observed on.
    pub channel: u8,

    /// Frequency in MHz.
    pub frequency_mhz: u16,

    /// Vendor name from OUI lookup.
    pub vendor: Option<String>,

    /// Unix timestamp (milliseconds).
    pub timestamp_ms: u64,
}

/// A detected deauthentication or disassociation event.
///
/// Emitted via Tauri event `"deauth-detected"` when the sniffer sees
/// deauth/disassoc frames on the air — useful for detecting attacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeauthEvent {
    /// MAC of the device that sent the deauth/disassoc.
    pub source_mac: String,

    /// Target MAC.
    pub dest_mac: String,

    /// BSSID from the frame header.
    pub bssid: String,

    /// 802.11 reason code (e.g. 7 = Class 3 frame from non-associated STA).
    pub reason_code: u16,

    /// Signal strength in dBm.
    pub rssi: i8,

    /// Channel the frame was observed on.
    pub channel: u8,

    /// Whether the deauth targeted broadcast (FF:FF:FF:FF:FF:FF).
    pub is_broadcast: bool,

    /// Vendor name of the source MAC.
    pub vendor: Option<String>,

    /// Unix timestamp (milliseconds).
    pub timestamp_ms: u64,
}

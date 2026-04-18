/**
 * Aether — Packet Capture Types
 *
 * TypeScript interfaces matching the Rust `BeaconFrame` and `CaptureStatus`
 * structs. These are the payloads received from the Tauri IPC event stream.
 *
 * @module types/capture
 */

/**
 * A parsed 802.11 Beacon frame emitted by the Rust packet sniffer.
 *
 * Received via Tauri event `"beacon-frame"` when a capture session is active.
 *
 * @example
 * ```typescript
 * import { listen } from '@tauri-apps/api/event';
 * import type { BeaconFrame } from '../types/capture';
 *
 * const unlisten = await listen<BeaconFrame>('beacon-frame', (event) => {
 *   const beacon = event.payload;
 *   console.log(`${beacon.ssid} (${beacon.bssid}) CH:${beacon.channel} ${beacon.rssi}dBm`);
 * });
 * ```
 */
export interface BeaconFrame {
    /** BSSID of the access point (MAC address, e.g. "AA:BB:CC:DD:EE:FF") */
    bssid: string;

    /** Network name (SSID). Empty string for hidden networks. */
    ssid: string;

    /** WiFi channel number (1–14 for 2.4 GHz, 36–165 for 5 GHz) */
    channel: number;

    /** Received Signal Strength Indicator in dBm (e.g. -45). Range: -100 to -20 */
    rssi: number;

    /** Frequency in MHz (e.g. 2437 for channel 6) */
    frequency_mhz: number;

    /** Vendor name from OUI lookup, or null if unknown */
    vendor: string | null;

    /** Unix timestamp in milliseconds when this frame was captured */
    timestamp_ms: number;
}

/**
 * Status payload returned by `start_capture` / `stop_capture` Tauri commands.
 */
export interface CaptureStatus {
    /** The interface being captured on */
    interface_name: string;

    /** Whether a capture is currently active */
    active: boolean;

    /** Human-readable status message */
    message: string;
}

/**
 * A discovered WiFi client station emitted by the Rust packet sniffer.
 *
 * Received via Tauri event `"station-info"` when a capture session is active.
 *
 * @example
 * ```typescript
 * import { listen } from '@tauri-apps/api/event';
 * import type { StationInfo } from '../types/capture';
 *
 * const unlisten = await listen<StationInfo>('station-info', (event) => {
 *   const station = event.payload;
 *   console.log(`${station.mac} → ${station.associated_bssid ?? 'unassociated'}`);
 * });
 * ```
 */
export interface StationInfo {
    /** Client device MAC address (e.g. "AA:BB:CC:DD:EE:FF") */
    mac: string;

    /** Associated AP BSSID, or null if the station is not associated */
    associated_bssid: string | null;

    /** Signal strength in dBm (e.g. -65). Range: -100 to -20 */
    rssi: number;

    /** Number of packets captured from this station */
    packet_count: number;

    /** List of SSIDs this station has probed for */
    probed_ssids: string[];

    /** Vendor name from OUI lookup, or null if not yet resolved */
    vendor: string | null;

    /** Unix timestamp in milliseconds when this station was last seen */
    timestamp_ms: number;
}

/**
 * A parsed 802.11 Probe Request frame detected by the sniffer.
 *
 * Received via Tauri event `"probe-request"` when the sniffer is active.
 */
export interface ProbeRequest {
    /** Source MAC of the probing device */
    source_mac: string;

    /** SSID being probed for. Empty string = wildcard probe */
    ssid: string;

    /** Signal strength in dBm */
    rssi: number;

    /** Channel the probe was observed on */
    channel: number;

    /** Frequency in MHz */
    frequency_mhz: number;

    /** Vendor name from OUI lookup, or null */
    vendor: string | null;

    /** Unix timestamp in milliseconds */
    timestamp_ms: number;
}

/**
 * A detected deauthentication or disassociation event.
 *
 * Received via Tauri event `"deauth-detected"` when the sniffer is active.
 */
export interface DeauthEvent {
    /** MAC of the device that sent the deauth/disassoc */
    source_mac: string;

    /** Target MAC */
    dest_mac: string;

    /** BSSID from the frame header */
    bssid: string;

    /** 802.11 reason code */
    reason_code: number;

    /** Signal strength in dBm */
    rssi: number;

    /** Channel the frame was observed on */
    channel: number;

    /** Whether the deauth targeted broadcast */
    is_broadcast: boolean;

    /** Vendor name of the source MAC */
    vendor: string | null;

    /** Unix timestamp in milliseconds */
    timestamp_ms: number;
}

/**
 * Error payload returned by Tauri commands on failure.
 */
export interface AetherError {
    /** Machine-readable error code (e.g. "CAPTURE_ERROR", "PERMISSION_DENIED") */
    code: string;

    /** Human-readable error message */
    message: string;
}

// ─────────────────────────────────────────────────
// Audit / Offensive Types
// ─────────────────────────────────────────────────

/**
 * Result of a deauthentication injection operation.
 */
export interface DeauthResult {
    success: boolean;
    message: string;
    bssid: string;
    packets_sent: number;
    packets_total: number;
    stopped_early: boolean;
}

/**
 * Result of a handshake capture operation.
 */
export interface HandshakeResult {
    success: boolean;
    message: string;
    pcap_path: string | null;
}

/**
 * Real-time status event for EAPOL capture operations.
 * Received via Tauri event `"eapol-status"` or `"deauth-status"`.
 */
export interface CaptureOperationStatus {
    /** Current phase: "setup" | "deauth" | "capturing" | "complete" | "error" | "stopped" */
    phase: string;
    /** Human-readable status message */
    message: string;
    /** Progress from 0.0 to 1.0 */
    progress: number;
    /** Number of deauth packets sent (optional, for deauth operations) */
    packets_sent?: number;
    /** Total deauth packets to send (optional, for deauth operations) */
    packets_total?: number;
}

/**
 * Represents a network interface discovered by the Rust backend.
 */
export interface NetworkInterface {
    name: string;
    mac_address: string | null;
    mode: "managed" | "monitor" | "unknown";
    is_up: boolean;
    is_wireless: boolean;
    driver: string | null;
    chipset: string | null;
    phy: string | null;
}

/**
 * Result of an interface mode change operation.
 */
export interface InterfaceModeResult {
    interface_name: string;
    previous_mode: string;
    current_mode: string;
    success: boolean;
    message: string;
}

// ─────────────────────────────────────────────────
// Network Scanner Types
// ─────────────────────────────────────────────────

/**
 * Host discovered via ping or ARP scan.
 * Returned by `invoke('ping_scan')` and `invoke('arp_scan')`.
 */
export interface HostInfo {
    /** IP address of the discovered host */
    ip: string;
    /** MAC address (available from ARP scans) */
    mac: string | null;
    /** Resolved hostname, or null */
    hostname: string | null;
    /** Hardware vendor from OUI lookup */
    vendor: string | null;
    /** Whether the host responded */
    is_up: boolean;
    /** Unix timestamp in milliseconds */
    timestamp_ms: number;
}

/**
 * Result of a port scan on a single port.
 * Returned by `invoke('port_scan')`.
 */
export interface PortResult {
    /** Target host IP */
    host: string;
    /** Port number */
    port: number;
    /** Protocol (tcp/udp) */
    protocol: string;
    /** Port state: open, closed, or filtered */
    state: string;
    /** Detected service name, or null */
    service: string | null;
    /** Detected service version, or null */
    version: string | null;
}

/**
 * A discovered network service (SSH, Telnet, etc.).
 * Returned by `invoke('ssh_scan')` and `invoke('telnet_scan')`.
 */
export interface ServiceInfo {
    /** Host IP address */
    host: string;
    /** Service port number */
    port: number;
    /** Service name (e.g. "ssh", "telnet") */
    service: string;
    /** Service version string, or null */
    version: string | null;
    /** Host MAC address, or null */
    mac: string | null;
    /** Hardware vendor, or null */
    vendor: string | null;
}

/**
 * Result of a MAC address spoof/restore operation.
 */
export interface MacSpoofResult {
    interface_name: string;
    original_mac: string;
    new_mac: string;
    vendor: string | null;
    success: boolean;
    message: string;
}

/**
 * A named SSID list used by beacon spam, evil portal, etc.
 */
export interface SsidList {
    name: string;
    ssids: string[];
}

/**
 * A saved access point with all known metadata.
 */
export interface SavedAp {
    bssid: string;
    ssid: string;
    channel: number;
    rssi: number;
    encryption: string | null;
    vendor: string | null;
    first_seen_ms: number;
    last_seen_ms: number;
    client_count: number;
    selected: boolean;
}

/**
 * Detailed AP info aggregated from multiple captures.
 */
export interface ApDetail {
    bssid: string;
    ssid: string;
    channel: number;
    frequency_mhz: number;
    rssi: number;
    rssi_history: number[];
    encryption: string | null;
    cipher: string | null;
    auth: string | null;
    vendor: string | null;
    client_count: number;
    clients: string[];
    first_seen_ms: number;
    last_seen_ms: number;
    beacon_count: number;
}

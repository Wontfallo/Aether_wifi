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

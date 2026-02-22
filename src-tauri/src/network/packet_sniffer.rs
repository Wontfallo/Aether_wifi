//! Packet sniffer — captures and parses raw 802.11 Beacon frames.
//!
//! This module uses libpcap to open a monitor-mode wireless interface
//! and captures raw 802.11 frames. It specifically targets Beacon frames
//! (management type 0, subtype 8) and extracts:
//!   - BSSID (from the 802.11 MAC header)
//!   - SSID  (from the SSID Information Element, Tag 0)
//!   - Channel (from DS Parameter Set IE Tag 3, or radiotap frequency)
//!   - RSSI  (Antenna Signal dBm from the Radiotap header)
//!
//! # Frame Structure (captured in monitor mode with Radiotap)
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │  Radiotap Header (variable length)               │
//! │  ├─ version: u8 (always 0)                       │
//! │  ├─ pad:     u8                                  │
//! │  ├─ length:  u16 LE  ← total radiotap length     │
//! │  ├─ present: u32 LE  ← bitmask of fields         │
//! │  └─ fields:  [varies by present flags]           │
//! ├──────────────────────────────────────────────────┤
//! │  802.11 MAC Header (24 bytes for mgmt frames)    │
//! │  ├─ Frame Control:   2 bytes                     │
//! │  ├─ Duration/ID:     2 bytes                     │
//! │  ├─ Address 1 (DA):  6 bytes  ← broadcast        │
//! │  ├─ Address 2 (SA):  6 bytes  ← BSSID            │
//! │  ├─ Address 3:       6 bytes  ← BSSID            │
//! │  └─ Sequence Ctrl:   2 bytes                     │
//! ├──────────────────────────────────────────────────┤
//! │  Beacon Frame Body                               │
//! │  ├─ Timestamp:       8 bytes                     │
//! │  ├─ Beacon Interval: 2 bytes                     │
//! │  ├─ Capability Info: 2 bytes                     │
//! │  └─ Tagged Parameters (IEs):                     │
//! │     ├─ Tag 0: SSID                               │
//! │     ├─ Tag 3: DS Parameter Set (channel)         │
//! │     └─ ...                                       │
//! └──────────────────────────────────────────────────┘
//! ```

use byteorder::{LittleEndian, ReadBytesExt};
use log::{debug, error, info, trace, warn};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::BeaconFrame;

// ─────────────────────────────────────────────────
// Constants: 802.11 Frame Control
// ─────────────────────────────────────────────────

/// 802.11 Management frame type (bits 2-3 of Frame Control byte 0).
const FRAME_TYPE_MGMT: u8 = 0;

/// Beacon subtype value (bits 4-7 of Frame Control byte 0).
const FRAME_SUBTYPE_BEACON: u8 = 8;

/// Size of the 802.11 Management frame MAC header (bytes).
const MGMT_MAC_HEADER_LEN: usize = 24;

/// Beacon frame body fixed fields: timestamp (8) + interval (2) + capability (2).
const BEACON_FIXED_FIELDS_LEN: usize = 12;

// ─────────────────────────────────────────────────
// Constants: Radiotap Present Flag Bits
// ─────────────────────────────────────────────────

/// Bit positions in the radiotap `present` bitmask.
const RT_TSFT: u32 = 0;
const RT_FLAGS: u32 = 1;
const RT_RATE: u32 = 2;
const RT_CHANNEL: u32 = 3;
const RT_FHSS: u32 = 4;
const RT_DBM_ANTSIGNAL: u32 = 5;
const RT_DBM_ANTNOISE: u32 = 6;
const RT_LOCK_QUALITY: u32 = 7;
const RT_TX_ATTENUATION: u32 = 8;
const RT_DB_TX_ATTENUATION: u32 = 9;
const RT_DBM_TX_POWER: u32 = 10;
const RT_ANTENNA: u32 = 11;
const RT_DB_ANTSIGNAL: u32 = 12;
const RT_DB_ANTNOISE: u32 = 13;
// Bit 31: extension flag (another 32-bit present word follows)
const RT_EXT: u32 = 31;

/// Radiotap field sizes and natural alignment requirements.
/// Format: (size_in_bytes, alignment)
///
/// Source: http://www.radiotap.org/ — field definitions
const RT_FIELD_INFO: [(usize, usize); 14] = [
    (8, 8), // 0: TSFT         — u64, align 8
    (1, 1), // 1: Flags        — u8,  align 1
    (1, 1), // 2: Rate         — u8,  align 1
    (4, 2), // 3: Channel      — u16 freq + u16 flags, align 2
    (2, 2), // 4: FHSS         — u16, align 2
    (1, 1), // 5: dBm Signal   — i8,  align 1
    (1, 1), // 6: dBm Noise    — i8,  align 1
    (2, 2), // 7: Lock Quality — u16, align 2
    (2, 2), // 8: TX Atten     — u16, align 2
    (2, 2), // 9: dB TX Atten  — u16, align 2
    (1, 1), // 10: dBm TX Pwr  — i8,  align 1
    (1, 1), // 11: Antenna     — u8,  align 1
    (1, 1), // 12: dB Signal   — u8,  align 1
    (1, 1), // 13: dB Noise    — u8,  align 1
];

// ─────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────

/// Handle for controlling an active capture session.
/// When `stop()` is called, the capture loop exits cleanly.
pub struct CaptureHandle {
    stop_flag: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    hopper_thread: Option<std::thread::JoinHandle<()>>,
}

impl CaptureHandle {
    /// Signal the capture thread to stop and wait for it to exit.
    pub fn stop(&mut self) {
        info!("Stopping capture...");
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.hopper_thread.take() {
            let _ = handle.join();
        }
        info!("Capture stopped.");
    }

    /// Check whether the capture is still running.
    pub fn is_active(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }
}

/// Start capturing beacon frames on the specified monitor-mode interface.
///
/// The capture runs in a background thread. Parsed [`BeaconFrame`] payloads
/// are sent to the provided callback, which the Tauri command layer uses
/// to emit IPC events.
///
/// Returns a [`CaptureHandle`] that can be used to stop the capture.
pub fn start_capture<F>(
    interface_name: &str,
    on_beacon: F,
) -> Result<CaptureHandle, crate::error::AetherError>
where
    F: Fn(BeaconFrame) + Send + 'static,
{
    info!("Opening pcap capture on interface '{}'...", interface_name);

    // Open the interface in monitor mode with pcap.
    // We use a 65535 snaplen to capture full frames including radiotap.
    // Promiscuous mode is irrelevant in monitor mode but we set it anyway.
    // Timeout of 100ms prevents busy-waiting while still being responsive.
    // Helper closure to create an inactive capture with common settings.
    let make_inactive = || -> Result<pcap::Capture<pcap::Inactive>, crate::error::AetherError> {
        pcap::Capture::from_device(interface_name)
            .map_err(|e| {
                crate::error::AetherError::CaptureError(format!(
                    "Failed to open device '{}': {}",
                    interface_name, e
                ))
            })
            .map(|c| c.snaplen(65535).promisc(true).timeout(100))
    };

    // On Linux, try opening with rfmon(true) first. If the interface is
    // *already* in monitor mode (common with Realtek/DKMS drivers), the
    // SIOCSIWMODE ioctl may fail. In that case, fall back to opening
    // without rfmon — the interface is already delivering radiotap frames.
    #[cfg(not(windows))]
    let cap = {
        let with_rfmon = make_inactive()?.rfmon(true).open();
        match with_rfmon {
            Ok(c) => {
                info!("Opened capture with rfmon=true on '{}'", interface_name);
                c
            }
            Err(rfmon_err) => {
                warn!(
                    "rfmon(true) failed on '{}': {}. Trying without rfmon \
                     (interface may already be in monitor mode).",
                    interface_name, rfmon_err
                );
                make_inactive()?.open().map_err(|e| {
                    crate::error::AetherError::CaptureError(format!(
                        "Failed to activate capture on '{}': {}. \
                         Ensure the interface is in monitor mode and you have root/sudo privileges.",
                        interface_name, e
                    ))
                })?
            }
        }
    };

    #[cfg(windows)]
    let cap = make_inactive()?.open().map_err(|e| {
        crate::error::AetherError::CaptureError(format!(
            "Failed to activate capture on '{}': {}. \
             Ensure the interface is in monitor mode.",
            interface_name, e
        ))
    })?;

    info!(
        "Capture opened on '{}'. Datalink: {:?}",
        interface_name,
        cap.get_datalink()
    );

    // Verify we're getting radiotap-encapsulated frames
    let datalink = cap.get_datalink();
    if datalink != pcap::Linktype::IEEE802_11_RADIOTAP {
        warn!(
            "Expected IEEE802_11_RADIOTAP datalink, got {:?}. \
             Beacon parsing may fail.",
            datalink
        );
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let iface_name = interface_name.to_string();

    let thread = std::thread::Builder::new()
        .name(format!("aether-capture-{}", interface_name))
        .spawn(move || {
            capture_loop(cap, &iface_name, stop_clone, on_beacon);
        })
        .map_err(|e| {
            crate::error::AetherError::CaptureError(format!(
                "Failed to spawn capture thread: {}",
                e
            ))
        })?;

    let stop_clone2 = stop_flag.clone();
    let iface_name2 = interface_name.to_string();

    let hopper_thread = std::thread::Builder::new()
        .name(format!("aether-hopper-{}", interface_name))
        .spawn(move || {
            // A comprehensive mix of 2.4GHz and 5GHz channels
            let channels = [1, 6, 11, 36, 40, 44, 48, 149, 153, 157, 161];
            let mut idx = 0;

            while !stop_clone2.load(Ordering::Relaxed) {
                let channel = channels[idx];
                // Non-blocking channel hop — suppress stdout/stderr
                let _ = std::process::Command::new("iw")
                    .arg("dev")
                    .arg(&iface_name2)
                    .arg("set")
                    .arg("channel")
                    .arg(channel.to_string())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output();

                idx = (idx + 1) % channels.len();

                // Wait ~200ms on each channel, but check stop flag every 50ms
                for _ in 0..4 {
                    if stop_clone2.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        })
        .ok(); // Ignoring spawn failures on hopper to keep capturing alive

    Ok(CaptureHandle {
        stop_flag,
        thread: Some(thread),
        hopper_thread,
    })
}

// ─────────────────────────────────────────────────
// Capture Loop
// ─────────────────────────────────────────────────

/// Main capture loop — runs in a dedicated thread.
fn capture_loop<F>(
    mut cap: pcap::Capture<pcap::Active>,
    interface_name: &str,
    stop_flag: Arc<AtomicBool>,
    on_beacon: F,
) where
    F: Fn(BeaconFrame),
{
    info!("Capture loop started on '{}'", interface_name);

    let mut packet_count: u64 = 0;
    let mut beacon_count: u64 = 0;

    while !stop_flag.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                packet_count += 1;
                trace!("Packet #{}: {} bytes", packet_count, packet.data.len());

                match parse_beacon_frame(packet.data) {
                    Some(beacon) => {
                        beacon_count += 1;
                        debug!(
                            "Beacon #{}: BSSID={} SSID=\"{}\" CH={} RSSI={}dBm",
                            beacon_count, beacon.bssid, beacon.ssid, beacon.channel, beacon.rssi
                        );
                        on_beacon(beacon);
                    }
                    None => {
                        // Not a beacon frame — silently skip
                    }
                }
            }
            Err(pcap::Error::TimeoutExpired) => {
                // Normal — no packet arrived within the timeout window.
                // Loop back to check the stop flag.
                continue;
            }
            Err(e) => {
                error!(
                    "Capture error on '{}': {}. Stopping capture.",
                    interface_name, e
                );
                break;
            }
        }
    }

    info!(
        "Capture loop ended on '{}'. Total packets: {}, Beacons: {}",
        interface_name, packet_count, beacon_count
    );
}

// ─────────────────────────────────────────────────
// Frame Parsers
// ─────────────────────────────────────────────────

/// Attempt to parse a raw captured packet as an 802.11 Beacon frame.
///
/// Returns `None` if the packet is not a beacon or is malformed.
fn parse_beacon_frame(data: &[u8]) -> Option<BeaconFrame> {
    // ── Step 1: Parse Radiotap Header ──
    // Minimum radiotap header: 8 bytes (version + pad + length + present)
    if data.len() < 8 {
        return None;
    }

    let rt_version = data[0];
    if rt_version != 0 {
        trace!("Unknown radiotap version: {}", rt_version);
        return None;
    }

    // Radiotap header length (little-endian u16 at offset 2)
    let rt_len = {
        let mut cursor = Cursor::new(&data[2..4]);
        cursor.read_u16::<LittleEndian>().ok()?
    } as usize;

    if rt_len > data.len() {
        trace!(
            "Radiotap length {} exceeds packet size {}",
            rt_len,
            data.len()
        );
        return None;
    }

    // Read the primary present flags (little-endian u32 at offset 4)
    let present = {
        let mut cursor = Cursor::new(&data[4..8]);
        cursor.read_u32::<LittleEndian>().ok()?
    };

    // Determine the offset where radiotap fields begin.
    // If extension bit (bit 31) is set, additional 32-bit present words follow.
    // We need to skip past all present words to reach the actual fields.
    let mut field_offset = 8_usize; // past version(1) + pad(1) + len(2) + present(4)

    // Skip extension present words
    let mut current_present = present;
    while current_present & (1 << RT_EXT) != 0 {
        if field_offset + 4 > rt_len {
            return None;
        }
        current_present = {
            let mut cursor = Cursor::new(&data[field_offset..field_offset + 4]);
            cursor.read_u32::<LittleEndian>().ok()?
        };
        field_offset += 4;
    }

    // ── Step 2: Extract RSSI and Channel from Radiotap fields ──
    let (rssi, frequency) = parse_radiotap_fields(data, present, field_offset, rt_len);

    // ── Step 3: Parse 802.11 MAC Header ──
    let dot11_start = rt_len;
    let dot11_data = data.get(dot11_start..)?;

    if dot11_data.len() < MGMT_MAC_HEADER_LEN {
        return None;
    }

    // Frame Control: 2 bytes (little-endian)
    let fc0 = dot11_data[0];
    // let fc1 = dot11_data[1]; // flags byte, not needed for beacon detection

    // Extract type and subtype from Frame Control byte 0:
    //   Bits 0-1: Protocol version (always 0)
    //   Bits 2-3: Type (0 = Management)
    //   Bits 4-7: Subtype (8 = Beacon)
    let frame_type = (fc0 >> 2) & 0x03;
    let frame_subtype = (fc0 >> 4) & 0x0F;

    if frame_type != FRAME_TYPE_MGMT || frame_subtype != FRAME_SUBTYPE_BEACON {
        return None; // Not a beacon frame
    }

    // ── Step 4: Extract BSSID ──
    // Address 2 (Source Address) at offset 10..16 = BSSID for beacon frames
    let bssid_bytes = &dot11_data[10..16];
    let bssid = format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bssid_bytes[0],
        bssid_bytes[1],
        bssid_bytes[2],
        bssid_bytes[3],
        bssid_bytes[4],
        bssid_bytes[5]
    );

    // ── Step 5: Parse Beacon Frame Body ──
    // The body starts after the 24-byte MAC header
    let body_start = MGMT_MAC_HEADER_LEN;
    let body = dot11_data.get(body_start..)?;

    // Skip fixed fields: Timestamp (8) + Beacon Interval (2) + Capability (2)
    if body.len() < BEACON_FIXED_FIELDS_LEN {
        return None;
    }
    let tagged_params = &body[BEACON_FIXED_FIELDS_LEN..];

    // ── Step 6: Parse Information Elements (Tagged Parameters) ──
    let (ssid, ie_channel) = parse_information_elements(tagged_params);

    // Determine channel: prefer the DS Parameter Set (IE Tag 3),
    // fall back to deriving from radiotap frequency
    let channel = ie_channel.unwrap_or_else(|| frequency.map(freq_to_channel).unwrap_or(0));

    let frequency_mhz = frequency.unwrap_or_else(|| channel_to_freq(channel));

    // Timestamp
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    Some(BeaconFrame {
        bssid,
        ssid,
        channel,
        rssi: rssi.unwrap_or(-100), // -100 dBm as "unknown" sentinel
        frequency_mhz,
        timestamp_ms,
    })
}

/// Walk the radiotap present flags and extract RSSI (dBm signal) and channel frequency.
///
/// This correctly handles the natural alignment requirements specified by
/// the radiotap standard for each field type.
fn parse_radiotap_fields(
    data: &[u8],
    present: u32,
    start_offset: usize,
    rt_len: usize,
) -> (Option<i8>, Option<u16>) {
    let mut offset = start_offset;
    let mut rssi: Option<i8> = None;
    let mut frequency: Option<u16> = None;

    for bit in 0..=13_u32 {
        if present & (1 << bit) == 0 {
            continue; // Field not present, skip
        }

        let (field_size, field_align) = RT_FIELD_INFO[bit as usize];

        // Apply natural alignment padding
        let align_remainder = offset % field_align;
        if align_remainder != 0 {
            offset += field_align - align_remainder;
        }

        // Bounds check
        if offset + field_size > rt_len || offset + field_size > data.len() {
            break;
        }

        match bit {
            RT_CHANNEL => {
                // Channel field: u16 frequency (LE) + u16 flags (LE)
                let mut cursor = Cursor::new(&data[offset..offset + 2]);
                frequency = cursor.read_u16::<LittleEndian>().ok();
            }
            RT_DBM_ANTSIGNAL => {
                // Antenna Signal: i8
                rssi = Some(data[offset] as i8);
            }
            _ => {
                // We don't need other fields, just advance past them
            }
        }

        offset += field_size;

        // Early exit if we have both values
        if rssi.is_some() && frequency.is_some() {
            break;
        }
    }

    (rssi, frequency)
}

/// Parse 802.11 Information Elements (Tagged Parameters) to extract SSID and channel.
///
/// Each IE has the structure:
/// ```text
/// | Tag Number (1 byte) | Tag Length (1 byte) | Tag Data (N bytes) |
/// ```
fn parse_information_elements(data: &[u8]) -> (String, Option<u8>) {
    let mut ssid = String::new();
    let mut channel: Option<u8> = None;
    let mut offset = 0;

    while offset + 2 <= data.len() {
        let tag_number = data[offset];
        let tag_length = data[offset + 1] as usize;
        offset += 2;

        if offset + tag_length > data.len() {
            break; // Malformed IE — truncated
        }

        let tag_data = &data[offset..offset + tag_length];

        match tag_number {
            0 => {
                // SSID Information Element
                // tag_length == 0 means hidden SSID
                ssid = String::from_utf8_lossy(tag_data).to_string();
            }
            3 => {
                // DS Parameter Set — single byte indicating the current channel
                if tag_length >= 1 {
                    channel = Some(tag_data[0]);
                }
            }
            _ => {
                // Other IEs — skip
            }
        }

        offset += tag_length;

        // Early exit if we have both
        if !ssid.is_empty() && channel.is_some() {
            break;
        }
    }

    (ssid, channel)
}

// ─────────────────────────────────────────────────
// Frequency ↔ Channel Conversion
// ─────────────────────────────────────────────────

/// Convert a WiFi frequency (MHz) to a channel number.
///
/// Covers 2.4 GHz (channels 1–14) and 5 GHz (channels 36–165).
fn freq_to_channel(freq: u16) -> u8 {
    match freq {
        // 2.4 GHz band: channels 1–13
        2412..=2472 => ((freq - 2407) / 5) as u8,
        // Channel 14 (Japan only)
        2484 => 14,
        // 5 GHz band (UNII-1 through UNII-3)
        5180..=5825 => ((freq - 5000) / 5) as u8,
        _ => 0, // Unknown
    }
}

/// Convert a WiFi channel number to frequency (MHz).
fn channel_to_freq(channel: u8) -> u16 {
    match channel {
        1..=13 => 2407 + (channel as u16) * 5,
        14 => 2484,
        36..=165 => 5000 + (channel as u16) * 5,
        _ => 0,
    }
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freq_to_channel_2_4ghz() {
        assert_eq!(freq_to_channel(2412), 1);
        assert_eq!(freq_to_channel(2437), 6);
        assert_eq!(freq_to_channel(2462), 11);
        assert_eq!(freq_to_channel(2472), 13);
        assert_eq!(freq_to_channel(2484), 14);
    }

    #[test]
    fn freq_to_channel_5ghz() {
        assert_eq!(freq_to_channel(5180), 36);
        assert_eq!(freq_to_channel(5200), 40);
        assert_eq!(freq_to_channel(5745), 149);
    }

    #[test]
    fn channel_to_freq_roundtrip() {
        for ch in 1..=13 {
            assert_eq!(freq_to_channel(channel_to_freq(ch)), ch);
        }
        assert_eq!(freq_to_channel(channel_to_freq(14)), 14);
        for ch in [36, 40, 44, 48, 52, 56, 60, 64, 149, 153, 157, 161, 165] {
            assert_eq!(freq_to_channel(channel_to_freq(ch)), ch);
        }
    }

    /// Construct a synthetic Beacon frame with Radiotap header and verify parsing.
    #[test]
    fn parse_synthetic_beacon() {
        // ── Build Radiotap Header ──
        // Present flags: TSFT(0) | Flags(1) | Rate(2) | Channel(3) | dBm_Signal(5)
        let present: u32 = (1 << RT_TSFT)
            | (1 << RT_FLAGS)
            | (1 << RT_RATE)
            | (1 << RT_CHANNEL)
            | (1 << RT_DBM_ANTSIGNAL);

        let mut packet: Vec<u8> = Vec::new();
        // Radiotap header
        packet.push(0); // version
        packet.push(0); // pad
                        // header length (will fill later)
        packet.push(0);
        packet.push(0);
        // present flags (LE)
        packet.extend_from_slice(&present.to_le_bytes());

        // TSFT: 8 bytes, align 8 (already at offset 8, which is aligned)
        packet.extend_from_slice(&[0u8; 8]);
        // Flags: 1 byte
        packet.push(0x00);
        // Rate: 1 byte
        packet.push(0x02); // 1 Mbps
                           // Channel: 4 bytes (freq LE + flags LE), align 2 → already aligned at 18
        let freq: u16 = 2437; // Channel 6
        packet.extend_from_slice(&freq.to_le_bytes());
        packet.extend_from_slice(&[0x00, 0x00]); // channel flags
                                                 // dBm Antenna Signal: 1 byte (i8, signed) → -45 dBm
        packet.push((-45i8) as u8);

        // Pad to make rt_len predictable
        let rt_len = packet.len() as u16;
        packet[2] = (rt_len & 0xFF) as u8;
        packet[3] = ((rt_len >> 8) & 0xFF) as u8;

        // ── Build 802.11 Beacon MAC Header (24 bytes) ──
        // Frame Control: Type=0 (Mgmt), Subtype=8 (Beacon)
        // FC byte 0: (subtype << 4) | (type << 2) | version = (8 << 4) | 0 | 0 = 0x80
        packet.push(0x80); // FC byte 0
        packet.push(0x00); // FC byte 1

        // Duration
        packet.extend_from_slice(&[0x00, 0x00]);

        // Address 1 (DA): Broadcast
        packet.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

        // Address 2 (SA = BSSID)
        let bssid = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        packet.extend_from_slice(&bssid);

        // Address 3 (BSSID again)
        packet.extend_from_slice(&bssid);

        // Sequence Control
        packet.extend_from_slice(&[0x00, 0x00]);

        // ── Build Beacon Frame Body ──
        // Timestamp: 8 bytes
        packet.extend_from_slice(&[0u8; 8]);
        // Beacon Interval: 2 bytes (100 TU = ~102.4ms)
        packet.extend_from_slice(&[0x64, 0x00]);
        // Capability Info: 2 bytes
        packet.extend_from_slice(&[0x31, 0x04]);

        // ── Tagged Parameters (IEs) ──
        // Tag 0: SSID = "TestNet"
        let ssid_bytes = b"TestNet";
        packet.push(0); // tag number
        packet.push(ssid_bytes.len() as u8); // tag length
        packet.extend_from_slice(ssid_bytes);

        // Tag 3: DS Parameter Set (Channel 6)
        packet.push(3); // tag number
        packet.push(1); // tag length
        packet.push(6); // channel number

        // ── Parse ──
        let beacon = parse_beacon_frame(&packet).expect("Should parse as a valid beacon");

        assert_eq!(beacon.bssid, "AA:BB:CC:DD:EE:FF");
        assert_eq!(beacon.ssid, "TestNet");
        assert_eq!(beacon.channel, 6);
        assert_eq!(beacon.rssi, -45);
        assert_eq!(beacon.frequency_mhz, 2437);
    }

    #[test]
    fn non_beacon_frame_returns_none() {
        // Build a minimal radiotap + non-beacon frame (Type=0, Subtype=0 = Association Request)
        let mut packet: Vec<u8> = Vec::new();
        // Radiotap header (minimal: 8 bytes, no present fields)
        packet.push(0); // version
        packet.push(0); // pad
        packet.extend_from_slice(&8u16.to_le_bytes()); // length = 8
        packet.extend_from_slice(&0u32.to_le_bytes()); // present = 0 (no fields)

        // 802.11 MAC header with Subtype=0 (not beacon)
        packet.push(0x00); // FC byte 0: subtype=0, type=0
        packet.push(0x00);
        packet.extend_from_slice(&[0u8; 22]); // Rest of MAC header

        assert!(parse_beacon_frame(&packet).is_none());
    }

    #[test]
    fn hidden_ssid_parsed() {
        // Build a beacon with zero-length SSID (hidden network)
        let mut packet: Vec<u8> = Vec::new();
        // Minimal radiotap header
        packet.push(0);
        packet.push(0);
        packet.extend_from_slice(&8u16.to_le_bytes());
        packet.extend_from_slice(&0u32.to_le_bytes());

        // 802.11 Beacon header
        packet.push(0x80); // FC: beacon
        packet.push(0x00);
        packet.extend_from_slice(&[0x00; 2]); // Duration
        packet.extend_from_slice(&[0xFF; 6]); // DA
        packet.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]); // BSSID
        packet.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]); // BSSID
        packet.extend_from_slice(&[0x00; 2]); // Seq ctrl

        // Beacon body fixed fields
        packet.extend_from_slice(&[0u8; 12]);

        // Tag 0: SSID with length 0 (hidden)
        packet.push(0);
        packet.push(0);

        // Tag 3: DS Parameter Set, channel 11
        packet.push(3);
        packet.push(1);
        packet.push(11);

        let beacon = parse_beacon_frame(&packet).expect("Should parse hidden-SSID beacon");
        assert_eq!(beacon.ssid, "");
        assert_eq!(beacon.channel, 11);
        assert_eq!(beacon.bssid, "11:22:33:44:55:66");
    }

    #[test]
    fn radiotap_alignment_with_gaps() {
        // Test that alignment padding is handled correctly.
        // Present: Flags(1) | Channel(3) | dBm_Signal(5)  (no TSFT, no Rate)
        let present: u32 = (1 << RT_FLAGS) | (1 << RT_CHANNEL) | (1 << RT_DBM_ANTSIGNAL);

        let mut radiotap: Vec<u8> = Vec::new();
        radiotap.push(0); // version
        radiotap.push(0); // pad
        radiotap.push(0); // length placeholder
        radiotap.push(0);
        radiotap.extend_from_slice(&present.to_le_bytes());

        // Offset 8: Flags (1 byte, align 1)
        radiotap.push(0x00);
        // Offset 9: Channel needs align 2 → pad to offset 10
        //   (no Rate present, so Channel comes right after Flags)
        radiotap.push(0x00); // alignment padding
                             // Offset 10: Channel freq (2437 = channel 6)
        let freq: u16 = 2437;
        radiotap.extend_from_slice(&freq.to_le_bytes());
        radiotap.extend_from_slice(&[0x00, 0x00]); // channel flags
                                                   // Offset 14: dBm Signal (no FHSS, so straight after Channel)
        radiotap.push((-72i8) as u8);

        let rt_len = radiotap.len() as u16;
        radiotap[2] = (rt_len & 0xFF) as u8;
        radiotap[3] = ((rt_len >> 8) & 0xFF) as u8;

        let (rssi, frequency) = parse_radiotap_fields(&radiotap, present, 8, rt_len as usize);

        assert_eq!(rssi, Some(-72));
        assert_eq!(frequency, Some(2437));
    }
}

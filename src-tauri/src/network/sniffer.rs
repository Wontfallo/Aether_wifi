//! General-purpose 802.11 frame sniffer.
//!
//! Runs alongside the main airodump-ng capture to detect specific frame
//! types that don't appear in CSV output: probe requests, deauthentication
//! frames, and other management/control frames.
//!
//! Uses libpcap directly on the monitor-mode interface.

use byteorder::{LittleEndian, ReadBytesExt};
use log::{debug, error, info, warn};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::oui::lookup_vendor;
use super::types::{DeauthEvent, ProbeRequest};

// 802.11 frame type/subtype constants
const FRAME_TYPE_MGMT: u8 = 0;
const FRAME_SUBTYPE_PROBE_REQ: u8 = 4;
const FRAME_SUBTYPE_DEAUTH: u8 = 12;
const FRAME_SUBTYPE_DISASSOC: u8 = 10;

const MGMT_MAC_HEADER_LEN: usize = 24;

// Radiotap constants (same as packet_sniffer.rs)
const RT_CHANNEL: u32 = 3;
const RT_DBM_ANTSIGNAL: u32 = 5;
const RT_EXT: u32 = 31;

const RT_FIELD_INFO: [(usize, usize); 14] = [
    (8, 8), // 0: TSFT
    (1, 1), // 1: Flags
    (1, 1), // 2: Rate
    (4, 2), // 3: Channel
    (2, 2), // 4: FHSS
    (1, 1), // 5: dBm Signal
    (1, 1), // 6: dBm Noise
    (2, 2), // 7: Lock Quality
    (2, 2), // 8: TX Atten
    (2, 2), // 9: dB TX Atten
    (1, 1), // 10: dBm TX Pwr
    (1, 1), // 11: Antenna
    (1, 1), // 12: dB Signal
    (1, 1), // 13: dB Noise
];

// ─────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────

/// Handle for controlling an active sniffer session.
pub struct SnifferHandle {
    stop_flag: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl SnifferHandle {
    pub fn stop(&mut self) {
        info!("Stopping sniffer...");
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        info!("Sniffer stopped.");
    }

    pub fn is_active(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }
}

/// Configuration for which frame types the sniffer should capture.
#[derive(Debug, Clone)]
pub struct SnifferConfig {
    pub probe_requests: bool,
    pub deauth_frames: bool,
}

impl Default for SnifferConfig {
    fn default() -> Self {
        Self {
            probe_requests: true,
            deauth_frames: true,
        }
    }
}

/// Start a general-purpose frame sniffer on a monitor-mode interface.
///
/// This opens its own pcap handle and can run alongside airodump-ng.
pub fn start_sniffer<P, D>(
    interface_name: &str,
    config: SnifferConfig,
    on_probe: P,
    on_deauth: D,
) -> Result<SnifferHandle, crate::error::AetherError>
where
    P: Fn(ProbeRequest) + Send + 'static,
    D: Fn(DeauthEvent) + Send + 'static,
{
    info!(
        "Starting frame sniffer on '{}' (probes={}, deauths={})",
        interface_name, config.probe_requests, config.deauth_frames
    );

    let cap = open_pcap(interface_name)?;
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let iface = interface_name.to_string();

    let thread = std::thread::Builder::new()
        .name(format!("aether-sniffer-{}", interface_name))
        .spawn(move || {
            sniffer_loop(cap, &iface, stop_clone, config, on_probe, on_deauth);
        })
        .map_err(|e| {
            crate::error::AetherError::CaptureError(format!(
                "Failed to spawn sniffer thread: {}",
                e
            ))
        })?;

    Ok(SnifferHandle {
        stop_flag,
        thread: Some(thread),
    })
}

fn open_pcap(
    interface_name: &str,
) -> Result<pcap::Capture<pcap::Active>, crate::error::AetherError> {
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

    #[cfg(not(windows))]
    let cap = {
        let with_rfmon = make_inactive()?.rfmon(true).open();
        match with_rfmon {
            Ok(c) => c,
            Err(_) => make_inactive()?
                .open()
                .map_err(|e| {
                    crate::error::AetherError::CaptureError(format!(
                        "Failed to open capture on '{}': {}",
                        interface_name, e
                    ))
                })?,
        }
    };

    #[cfg(windows)]
    let cap = make_inactive()?.open().map_err(|e| {
        crate::error::AetherError::CaptureError(format!(
            "Failed to open capture on '{}': {}",
            interface_name, e
        ))
    })?;

    Ok(cap)
}

// ─────────────────────────────────────────────────
// Sniffer Loop
// ─────────────────────────────────────────────────

fn sniffer_loop<P, D>(
    mut cap: pcap::Capture<pcap::Active>,
    interface_name: &str,
    stop_flag: Arc<AtomicBool>,
    config: SnifferConfig,
    on_probe: P,
    on_deauth: D,
) where
    P: Fn(ProbeRequest),
    D: Fn(DeauthEvent),
{
    info!("Sniffer loop started on '{}'", interface_name);

    let mut probe_count: u64 = 0;
    let mut deauth_count: u64 = 0;

    while !stop_flag.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                let data = packet.data;

                let parsed = match parse_frame_header(data) {
                    Some(h) => h,
                    None => continue,
                };

                if parsed.frame_type != FRAME_TYPE_MGMT {
                    continue;
                }

                match parsed.frame_subtype {
                    FRAME_SUBTYPE_PROBE_REQ if config.probe_requests => {
                        if let Some(probe) = parse_probe_request(data, &parsed) {
                            probe_count += 1;
                            debug!(
                                "Probe #{}: {} → \"{}\" RSSI={}dBm",
                                probe_count, probe.source_mac, probe.ssid, probe.rssi
                            );
                            on_probe(probe);
                        }
                    }
                    FRAME_SUBTYPE_DEAUTH if config.deauth_frames => {
                        if let Some(deauth) = parse_deauth_frame(data, &parsed) {
                            deauth_count += 1;
                            warn!(
                                "Deauth #{}: {} → {} reason={} RSSI={}dBm",
                                deauth_count,
                                deauth.source_mac,
                                deauth.dest_mac,
                                deauth.reason_code,
                                deauth.rssi
                            );
                            on_deauth(deauth);
                        }
                    }
                    FRAME_SUBTYPE_DISASSOC if config.deauth_frames => {
                        if let Some(deauth) = parse_disassoc_frame(data, &parsed) {
                            deauth_count += 1;
                            warn!(
                                "Disassoc #{}: {} → {} reason={} RSSI={}dBm",
                                deauth_count,
                                deauth.source_mac,
                                deauth.dest_mac,
                                deauth.reason_code,
                                deauth.rssi
                            );
                            on_deauth(deauth);
                        }
                    }
                    _ => {}
                }
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                error!("Sniffer error on '{}': {}. Stopping.", interface_name, e);
                break;
            }
        }
    }

    info!(
        "Sniffer loop ended on '{}'. Probes: {}, Deauths: {}",
        interface_name, probe_count, deauth_count
    );
}

// ─────────────────────────────────────────────────
// Frame Header Parser
// ─────────────────────────────────────────────────

struct FrameHeader {
    rt_len: usize,
    rssi: Option<i8>,
    frequency: Option<u16>,
    frame_type: u8,
    frame_subtype: u8,
}

fn parse_frame_header(data: &[u8]) -> Option<FrameHeader> {
    if data.len() < 8 {
        return None;
    }
    if data[0] != 0 {
        return None; // radiotap version must be 0
    }

    let rt_len = {
        let mut cursor = Cursor::new(&data[2..4]);
        cursor.read_u16::<LittleEndian>().ok()?
    } as usize;

    if rt_len > data.len() {
        return None;
    }

    let present = {
        let mut cursor = Cursor::new(&data[4..8]);
        cursor.read_u32::<LittleEndian>().ok()?
    };

    let mut field_offset = 8_usize;
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

    let (rssi, frequency) = parse_radiotap_fields(data, present, field_offset, rt_len);

    let dot11_start = rt_len;
    let dot11_data = data.get(dot11_start..)?;
    if dot11_data.len() < 2 {
        return None;
    }

    let fc0 = dot11_data[0];
    let frame_type = (fc0 >> 2) & 0x03;
    let frame_subtype = (fc0 >> 4) & 0x0F;

    Some(FrameHeader {
        rt_len,
        rssi,
        frequency,
        frame_type,
        frame_subtype,
    })
}

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
            continue;
        }

        let (field_size, field_align) = RT_FIELD_INFO[bit as usize];
        let align_remainder = offset % field_align;
        if align_remainder != 0 {
            offset += field_align - align_remainder;
        }

        if offset + field_size > rt_len || offset + field_size > data.len() {
            break;
        }

        match bit {
            RT_CHANNEL => {
                let mut cursor = Cursor::new(&data[offset..offset + 2]);
                frequency = cursor.read_u16::<LittleEndian>().ok();
            }
            RT_DBM_ANTSIGNAL => {
                rssi = Some(data[offset] as i8);
            }
            _ => {}
        }

        offset += field_size;
        if rssi.is_some() && frequency.is_some() {
            break;
        }
    }

    (rssi, frequency)
}

fn freq_to_channel(freq: u16) -> u8 {
    match freq {
        2412..=2472 => ((freq - 2407) / 5) as u8,
        2484 => 14,
        5180..=5825 => ((freq - 5000) / 5) as u8,
        _ => 0,
    }
}

fn format_mac(bytes: &[u8]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

// ─────────────────────────────────────────────────
// Probe Request Parser
// ─────────────────────────────────────────────────

fn parse_probe_request(data: &[u8], header: &FrameHeader) -> Option<ProbeRequest> {
    let dot11 = data.get(header.rt_len..)?;
    if dot11.len() < MGMT_MAC_HEADER_LEN {
        return None;
    }

    // Source address is Address 2 (offset 10..16)
    let source_mac = format_mac(&dot11[10..16]);

    // Probe request body starts after the 24-byte MAC header
    // (no fixed fields — directly starts with tagged parameters)
    let body = dot11.get(MGMT_MAC_HEADER_LEN..)?;

    // Parse SSID from tagged params (Tag 0)
    let ssid = parse_ssid_ie(body);

    let channel = header
        .frequency
        .map(freq_to_channel)
        .unwrap_or(0);

    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    Some(ProbeRequest {
        source_mac: source_mac.clone(),
        ssid,
        rssi: header.rssi.unwrap_or(-100),
        channel,
        frequency_mhz: header.frequency.unwrap_or(0),
        vendor: lookup_vendor(&source_mac).map(|s| s.to_string()),
        timestamp_ms,
    })
}

// ─────────────────────────────────────────────────
// Deauth Frame Parser
// ─────────────────────────────────────────────────

fn parse_deauth_frame(data: &[u8], header: &FrameHeader) -> Option<DeauthEvent> {
    let dot11 = data.get(header.rt_len..)?;
    if dot11.len() < MGMT_MAC_HEADER_LEN + 2 {
        return None; // Need at least MAC header + 2-byte reason code
    }

    let dest_mac = format_mac(&dot11[4..10]);
    let source_mac = format_mac(&dot11[10..16]);
    let bssid = format_mac(&dot11[16..22]);

    // Reason code is the first 2 bytes of the frame body
    let reason_code = {
        let body = &dot11[MGMT_MAC_HEADER_LEN..];
        let mut cursor = Cursor::new(&body[0..2]);
        cursor.read_u16::<LittleEndian>().ok()?
    };

    let channel = header
        .frequency
        .map(freq_to_channel)
        .unwrap_or(0);

    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    Some(DeauthEvent {
        source_mac: source_mac.clone(),
        dest_mac,
        bssid,
        reason_code,
        rssi: header.rssi.unwrap_or(-100),
        channel,
        is_broadcast: dot11[4..10] == [0xFF; 6],
        vendor: lookup_vendor(&source_mac).map(|s| s.to_string()),
        timestamp_ms,
    })
}

/// Parse a disassociation frame (same structure as deauth, different subtype).
fn parse_disassoc_frame(data: &[u8], header: &FrameHeader) -> Option<DeauthEvent> {
    // Disassociation has the same frame body layout as deauthentication
    parse_deauth_frame(data, header)
}

// ─────────────────────────────────────────────────
// IE Helpers
// ─────────────────────────────────────────────────

fn parse_ssid_ie(tagged_params: &[u8]) -> String {
    let mut offset = 0;
    while offset + 2 <= tagged_params.len() {
        let tag = tagged_params[offset];
        let len = tagged_params[offset + 1] as usize;
        offset += 2;

        if offset + len > tagged_params.len() {
            break;
        }

        if tag == 0 {
            return String::from_utf8_lossy(&tagged_params[offset..offset + len]).to_string();
        }

        offset += len;
    }
    String::new()
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn build_minimal_radiotap(rssi: i8, freq: u16) -> Vec<u8> {
        let present: u32 = (1 << RT_CHANNEL) | (1 << RT_DBM_ANTSIGNAL);
        let mut rt = Vec::new();
        rt.push(0); // version
        rt.push(0); // pad
        rt.push(0); // length placeholder
        rt.push(0);
        rt.extend_from_slice(&present.to_le_bytes());

        // Channel: 4 bytes (freq LE + flags LE), needs align 2 → at offset 8, already aligned
        rt.extend_from_slice(&freq.to_le_bytes());
        rt.extend_from_slice(&[0x00, 0x00]);
        // dBm Signal: 1 byte
        rt.push(rssi as u8);

        let len = rt.len() as u16;
        rt[2] = (len & 0xFF) as u8;
        rt[3] = ((len >> 8) & 0xFF) as u8;
        rt
    }

    fn build_mgmt_header(subtype: u8, sa: &[u8; 6], da: &[u8; 6], bssid: &[u8; 6]) -> Vec<u8> {
        let mut hdr = Vec::new();
        // FC byte 0: (subtype << 4) | (type << 2)  where type=0 (mgmt)
        hdr.push(subtype << 4);
        hdr.push(0x00); // FC byte 1
        hdr.extend_from_slice(&[0x00, 0x00]); // Duration
        hdr.extend_from_slice(da);     // Address 1 (DA)
        hdr.extend_from_slice(sa);     // Address 2 (SA)
        hdr.extend_from_slice(bssid);  // Address 3 (BSSID)
        hdr.extend_from_slice(&[0x00, 0x00]); // Sequence ctrl
        hdr
    }

    #[test]
    fn parse_probe_request_basic() {
        let mut packet = build_minimal_radiotap(-55, 2437);
        let sa = [0xAA, 0xBB, 0xCC, 0x11, 0x22, 0x33];
        let da = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]; // broadcast
        let bssid = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        packet.extend(build_mgmt_header(FRAME_SUBTYPE_PROBE_REQ, &sa, &da, &bssid));

        // SSID IE: "TestSSID"
        let ssid = b"TestSSID";
        packet.push(0); // tag 0
        packet.push(ssid.len() as u8);
        packet.extend_from_slice(ssid);

        let header = parse_frame_header(&packet).unwrap();
        assert_eq!(header.frame_type, FRAME_TYPE_MGMT);
        assert_eq!(header.frame_subtype, FRAME_SUBTYPE_PROBE_REQ);

        let probe = parse_probe_request(&packet, &header).unwrap();
        assert_eq!(probe.source_mac, "AA:BB:CC:11:22:33");
        assert_eq!(probe.ssid, "TestSSID");
        assert_eq!(probe.rssi, -55);
        assert_eq!(probe.channel, 6);
    }

    #[test]
    fn parse_probe_request_wildcard() {
        let mut packet = build_minimal_radiotap(-70, 2412);
        let sa = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let da = [0xFF; 6];
        let bssid = [0xFF; 6];
        packet.extend(build_mgmt_header(FRAME_SUBTYPE_PROBE_REQ, &sa, &da, &bssid));

        // SSID IE with length 0 → wildcard probe
        packet.push(0);
        packet.push(0);

        let header = parse_frame_header(&packet).unwrap();
        let probe = parse_probe_request(&packet, &header).unwrap();
        assert_eq!(probe.ssid, "");
        assert_eq!(probe.channel, 1);
    }

    #[test]
    fn parse_deauth_basic() {
        let mut packet = build_minimal_radiotap(-40, 2462);
        let sa = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        let da = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let bssid = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        packet.extend(build_mgmt_header(FRAME_SUBTYPE_DEAUTH, &sa, &da, &bssid));

        // Reason code: 7 = Class 3 frame received from nonassociated STA
        packet.extend_from_slice(&7u16.to_le_bytes());

        let header = parse_frame_header(&packet).unwrap();
        assert_eq!(header.frame_subtype, FRAME_SUBTYPE_DEAUTH);

        let deauth = parse_deauth_frame(&packet, &header).unwrap();
        assert_eq!(deauth.source_mac, "DE:AD:BE:EF:00:01");
        assert_eq!(deauth.dest_mac, "11:22:33:44:55:66");
        assert_eq!(deauth.reason_code, 7);
        assert_eq!(deauth.rssi, -40);
        assert_eq!(deauth.channel, 11);
        assert!(!deauth.is_broadcast);
    }

    #[test]
    fn parse_deauth_broadcast() {
        let mut packet = build_minimal_radiotap(-30, 2437);
        let sa = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let da = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]; // broadcast
        let bssid = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        packet.extend(build_mgmt_header(FRAME_SUBTYPE_DEAUTH, &sa, &da, &bssid));
        packet.extend_from_slice(&1u16.to_le_bytes()); // reason: Unspecified

        let header = parse_frame_header(&packet).unwrap();
        let deauth = parse_deauth_frame(&packet, &header).unwrap();
        assert!(deauth.is_broadcast);
        assert_eq!(deauth.reason_code, 1);
    }

    #[test]
    fn parse_disassoc_as_deauth_event() {
        let mut packet = build_minimal_radiotap(-50, 5180);
        let sa = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let da = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let bssid = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        packet.extend(build_mgmt_header(FRAME_SUBTYPE_DISASSOC, &sa, &da, &bssid));
        packet.extend_from_slice(&8u16.to_le_bytes()); // reason: Disassoc leaving BSS

        let header = parse_frame_header(&packet).unwrap();
        assert_eq!(header.frame_subtype, FRAME_SUBTYPE_DISASSOC);

        let event = parse_disassoc_frame(&packet, &header).unwrap();
        assert_eq!(event.source_mac, "00:11:22:33:44:55");
        assert_eq!(event.reason_code, 8);
        assert_eq!(event.channel, 36);
    }

    #[test]
    fn non_mgmt_frame_returns_none() {
        let mut packet = build_minimal_radiotap(-60, 2437);
        // Data frame: type=2, subtype=0 → FC byte 0 = (0 << 4) | (2 << 2) = 0x08
        packet.push(0x08);
        packet.push(0x00);
        packet.extend_from_slice(&[0u8; 22]);

        let header = parse_frame_header(&packet).unwrap();
        assert_ne!(header.frame_type, FRAME_TYPE_MGMT);
    }

    #[test]
    fn truncated_packet_returns_none() {
        // Only radiotap header, no 802.11 data
        let packet = build_minimal_radiotap(-50, 2437);
        let header = parse_frame_header(&packet);
        // Should get the header parsed (it's just radiotap + FC)
        // but there shouldn't be enough for a full mgmt frame
        assert!(header.is_none() || header.unwrap().frame_type != FRAME_TYPE_MGMT
            || parse_probe_request(&packet, &parse_frame_header(&packet).unwrap()).is_none());
    }
}

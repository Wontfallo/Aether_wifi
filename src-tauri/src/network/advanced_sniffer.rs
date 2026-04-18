//! Advanced 802.11 sniffer features for Aether-Core.
//!
//! Extends the base pcap capture engine with:
//! - Packet monitoring (frame counts per channel/second)
//! - Raw packet capture (all 802.11 frame metadata)
//! - PMKID passive extraction (hashcat -m 22000 format)
//! - Pwnagotchi beacon detection (vendor-specific IE with JSON)
//! - SAE commit/confirm sniffing (WPA3 auth frames)
//! - MAC address tracking across channels

use byteorder::{LittleEndian, ReadBytesExt};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

// ─────────────────────────────────────────────────
// Types (module-local, not in types.rs)
// ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketStats {
    pub channel: u8,
    pub total_frames: u64,
    pub management_frames: u64,
    pub control_frames: u64,
    pub data_frames: u64,
    pub frames_per_second: f64,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFrame {
    pub frame_type: String,
    pub subtype: String,
    pub addr1: Option<String>,
    pub addr2: Option<String>,
    pub addr3: Option<String>,
    pub size: usize,
    pub rssi: Option<i8>,
    pub channel: Option<u8>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmkidCapture {
    pub bssid: String,
    pub client_mac: String,
    pub pmkid: String,
    pub ssid: String,
    pub hashcat_line: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwnagotchiInfo {
    pub name: String,
    pub version: String,
    pub uptime: u64,
    pub epoch: u64,
    pub bssid: String,
    pub channel: Option<u8>,
    pub rssi: Option<i8>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaeFrame {
    pub source: String,
    pub destination: String,
    pub bssid: String,
    pub seq_num: u16,
    pub is_commit: bool,
    pub is_confirm: bool,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacTrackEntry {
    pub mac: String,
    pub rssi: Option<i8>,
    pub channel: Option<u8>,
    pub frame_type: String,
    pub role: String,
    pub timestamp_ms: u64,
}

// ─────────────────────────────────────────────────
// 802.11 / Radiotap constants
// ─────────────────────────────────────────────────

const FRAME_TYPE_MGMT: u8 = 0;
const FRAME_TYPE_CTRL: u8 = 1;
const FRAME_TYPE_DATA: u8 = 2;

const FRAME_SUBTYPE_BEACON: u8 = 8;
const FRAME_SUBTYPE_AUTH: u8 = 11;

const MGMT_MAC_HEADER_LEN: usize = 24;

// EAPOL ethertype
const EAPOL_ETHERTYPE: u16 = 0x888e;

// SAE algorithm number
const SAE_ALGORITHM_NUMBER: u16 = 3;

// SAE sequence numbers
const SAE_SEQ_COMMIT: u16 = 1;
const SAE_SEQ_CONFIRM: u16 = 2;

// Vendor-specific IE tag
const IE_VENDOR_SPECIFIC: u8 = 221;

// Radiotap field constants (mirrored from sniffer.rs)
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
// Internal parsed header
// ─────────────────────────────────────────────────

pub(crate) struct FrameHeader {
    pub rt_len: usize,
    pub rssi: Option<i8>,
    pub frequency: Option<u16>,
    pub frame_type: u8,
    pub frame_subtype: u8,
}

// ─────────────────────────────────────────────────
// Radiotap / Frame parsing helpers (public for tests)
// ─────────────────────────────────────────────────

pub fn parse_frame_header(data: &[u8]) -> Option<FrameHeader> {
    if data.len() < 8 {
        return None;
    }
    if data[0] != 0 {
        return None;
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

    let dot11_data = data.get(rt_len..)?;
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

pub fn freq_to_channel(freq: u16) -> u8 {
    match freq {
        2412..=2472 => ((freq - 2407) / 5) as u8,
        2484 => 14,
        5180..=5825 => ((freq - 5000) / 5) as u8,
        _ => 0,
    }
}

pub fn format_mac(bytes: &[u8]) -> String {
    if bytes.len() < 6 {
        return String::from("??:??:??:??:??:??");
    }
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn frame_type_str(t: u8) -> &'static str {
    match t {
        FRAME_TYPE_MGMT => "management",
        FRAME_TYPE_CTRL => "control",
        FRAME_TYPE_DATA => "data",
        _ => "unknown",
    }
}

fn mgmt_subtype_str(s: u8) -> &'static str {
    match s {
        0 => "association_request",
        1 => "association_response",
        2 => "reassociation_request",
        3 => "reassociation_response",
        4 => "probe_request",
        5 => "probe_response",
        6 => "timing_advertisement",
        8 => "beacon",
        9 => "atim",
        10 => "disassociation",
        11 => "authentication",
        12 => "deauthentication",
        13 => "action",
        14 => "action_no_ack",
        _ => "reserved",
    }
}

fn ctrl_subtype_str(s: u8) -> &'static str {
    match s {
        7 => "control_wrapper",
        8 => "block_ack_request",
        9 => "block_ack",
        10 => "ps_poll",
        11 => "rts",
        12 => "cts",
        13 => "ack",
        14 => "cf_end",
        15 => "cf_end_cf_ack",
        _ => "reserved",
    }
}

fn data_subtype_str(s: u8) -> &'static str {
    match s {
        0 => "data",
        1 => "data_cf_ack",
        2 => "data_cf_poll",
        3 => "data_cf_ack_poll",
        4 => "null",
        5 => "cf_ack",
        6 => "cf_poll",
        7 => "cf_ack_poll",
        8 => "qos_data",
        9 => "qos_data_cf_ack",
        10 => "qos_data_cf_poll",
        11 => "qos_data_cf_ack_poll",
        12 => "qos_null",
        _ => "reserved",
    }
}

fn subtype_str(frame_type: u8, subtype: u8) -> String {
    match frame_type {
        FRAME_TYPE_MGMT => mgmt_subtype_str(subtype).to_string(),
        FRAME_TYPE_CTRL => ctrl_subtype_str(subtype).to_string(),
        FRAME_TYPE_DATA => data_subtype_str(subtype).to_string(),
        _ => format!("unknown_{}", subtype),
    }
}

/// Extract up to three MAC addresses from an 802.11 frame after the radiotap header.
/// Returns (addr1, addr2, addr3).
fn extract_addresses(dot11: &[u8]) -> (Option<String>, Option<String>, Option<String>) {
    let addr1 = if dot11.len() >= 10 {
        Some(format_mac(&dot11[4..10]))
    } else {
        None
    };
    let addr2 = if dot11.len() >= 16 {
        Some(format_mac(&dot11[10..16]))
    } else {
        None
    };
    let addr3 = if dot11.len() >= 22 {
        Some(format_mac(&dot11[16..22]))
    } else {
        None
    };
    (addr1, addr2, addr3)
}

// ─────────────────────────────────────────────────
// pcap opener (replicates sniffer.rs open_pcap)
// ─────────────────────────────────────────────────

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
            Err(_) => make_inactive()?.open().map_err(|e| {
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
// A. Packet Monitor
// ─────────────────────────────────────────────────

pub fn start_packet_monitor<F>(
    interface_name: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) -> Result<std::thread::JoinHandle<()>, crate::error::AetherError>
where
    F: Fn(PacketStats) + Send + 'static,
{
    info!("Starting packet monitor on '{}'", interface_name);
    let mut cap = open_pcap(interface_name)?;
    let iface = interface_name.to_string();

    let handle = std::thread::Builder::new()
        .name(format!("aether-pktmon-{}", interface_name))
        .spawn(move || {
            packet_monitor_loop(&mut cap, &iface, stop, callback);
        })
        .map_err(|e| {
            crate::error::AetherError::CaptureError(format!("Failed to spawn thread: {}", e))
        })?;

    Ok(handle)
}

fn packet_monitor_loop<F>(
    cap: &mut pcap::Capture<pcap::Active>,
    _iface: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) where
    F: Fn(PacketStats),
{
    let mut total: u64 = 0;
    let mut mgmt: u64 = 0;
    let mut ctrl: u64 = 0;
    let mut data: u64 = 0;
    let mut last_emit = Instant::now();

    while !stop.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(hdr) = parse_frame_header(packet.data) {
                    total += 1;
                    match hdr.frame_type {
                        FRAME_TYPE_MGMT => mgmt += 1,
                        FRAME_TYPE_CTRL => ctrl += 1,
                        FRAME_TYPE_DATA => data += 1,
                        _ => {}
                    }

                    let elapsed = last_emit.elapsed();
                    if elapsed.as_secs_f64() >= 1.0 {
                        let fps = total as f64 / elapsed.as_secs_f64();
                        let channel = hdr
                            .frequency
                            .map(freq_to_channel)
                            .unwrap_or(0);
                        callback(PacketStats {
                            channel,
                            total_frames: total,
                            management_frames: mgmt,
                            control_frames: ctrl,
                            data_frames: data,
                            frames_per_second: fps,
                            timestamp_ms: now_ms(),
                        });
                        total = 0;
                        mgmt = 0;
                        ctrl = 0;
                        data = 0;
                        last_emit = Instant::now();
                    }
                }
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                error!("Packet monitor error: {}. Stopping.", e);
                break;
            }
        }
    }
    info!("Packet monitor stopped.");
}

// ─────────────────────────────────────────────────
// B. Raw Packet Capture
// ─────────────────────────────────────────────────

pub fn start_raw_capture<F>(
    interface_name: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) -> Result<std::thread::JoinHandle<()>, crate::error::AetherError>
where
    F: Fn(RawFrame) + Send + 'static,
{
    info!("Starting raw capture on '{}'", interface_name);
    let mut cap = open_pcap(interface_name)?;
    let iface = interface_name.to_string();

    let handle = std::thread::Builder::new()
        .name(format!("aether-rawcap-{}", interface_name))
        .spawn(move || {
            raw_capture_loop(&mut cap, &iface, stop, callback);
        })
        .map_err(|e| {
            crate::error::AetherError::CaptureError(format!("Failed to spawn thread: {}", e))
        })?;

    Ok(handle)
}

fn raw_capture_loop<F>(
    cap: &mut pcap::Capture<pcap::Active>,
    _iface: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) where
    F: Fn(RawFrame),
{
    while !stop.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(frame) = parse_raw_frame(packet.data) {
                    callback(frame);
                }
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                error!("Raw capture error: {}. Stopping.", e);
                break;
            }
        }
    }
    info!("Raw capture stopped.");
}

pub fn parse_raw_frame(data: &[u8]) -> Option<RawFrame> {
    let hdr = parse_frame_header(data)?;
    let dot11 = data.get(hdr.rt_len..)?;
    let (addr1, addr2, addr3) = extract_addresses(dot11);
    let channel = hdr.frequency.map(freq_to_channel);

    Some(RawFrame {
        frame_type: frame_type_str(hdr.frame_type).to_string(),
        subtype: subtype_str(hdr.frame_type, hdr.frame_subtype),
        addr1,
        addr2,
        addr3,
        size: data.len(),
        rssi: hdr.rssi,
        channel,
        timestamp_ms: now_ms(),
    })
}

// ─────────────────────────────────────────────────
// C. PMKID Passive Capture
// ─────────────────────────────────────────────────

pub fn start_pmkid_capture<F>(
    interface_name: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) -> Result<std::thread::JoinHandle<()>, crate::error::AetherError>
where
    F: Fn(PmkidCapture) + Send + 'static,
{
    info!("Starting PMKID capture on '{}'", interface_name);
    let mut cap = open_pcap(interface_name)?;
    let iface = interface_name.to_string();

    let handle = std::thread::Builder::new()
        .name(format!("aether-pmkid-{}", interface_name))
        .spawn(move || {
            pmkid_capture_loop(&mut cap, &iface, stop, callback);
        })
        .map_err(|e| {
            crate::error::AetherError::CaptureError(format!("Failed to spawn thread: {}", e))
        })?;

    Ok(handle)
}

fn pmkid_capture_loop<F>(
    cap: &mut pcap::Capture<pcap::Active>,
    _iface: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) where
    F: Fn(PmkidCapture),
{
    while !stop.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(capture) = try_extract_pmkid(packet.data) {
                    info!("PMKID captured: {}", capture.hashcat_line);
                    callback(capture);
                }
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                error!("PMKID capture error: {}. Stopping.", e);
                break;
            }
        }
    }
    info!("PMKID capture stopped.");
}

/// Try to extract a PMKID from a raw pcap frame.
///
/// EAPOL frames in monitor mode appear as data frames (type 2).
/// The LLC/SNAP header after the 802.11 header carries ethertype 0x888e.
/// We look for EAPOL-Key message 1 (AP → client) which carries the PMKID
/// in the Key Data field inside an RSN PMKID-List KDE (OUI 00:0F:AC, type 4).
pub fn try_extract_pmkid(data: &[u8]) -> Option<PmkidCapture> {
    let hdr = parse_frame_header(data)?;
    if hdr.frame_type != FRAME_TYPE_DATA {
        return None;
    }

    let dot11 = data.get(hdr.rt_len..)?;
    if dot11.len() < 26 {
        return None;
    }

    // Determine 802.11 header length (QoS data adds 2 bytes)
    let fc0 = dot11[0];
    let is_qos = (fc0 >> 4) & 0x0F >= 8; // QoS subtype
    let dot11_hdr_len = if is_qos { 26 } else { 24 };

    // LLC/SNAP header follows the 802.11 header
    let llc_start = dot11_hdr_len;
    if dot11.len() < llc_start + 8 {
        return None;
    }

    let llc = &dot11[llc_start..];
    // LLC/SNAP: AA AA 03 00 00 00 + 2 byte ethertype
    if llc[0] != 0xAA || llc[1] != 0xAA || llc[2] != 0x03 {
        return None;
    }

    let ethertype = u16::from_be_bytes([llc[6], llc[7]]);
    if ethertype != EAPOL_ETHERTYPE {
        return None;
    }

    let eapol = &llc[8..];
    if let Some(pmkid_result) = parse_eapol_for_pmkid(eapol) {
        // Extract MACs from the 802.11 header
        // For data frames from AP: addr1=DA(client), addr2=BSSID, addr3=SA(BSSID)
        // Using To-DS / From-DS bits to determine addressing
        let fc1 = dot11[1];
        let to_ds = fc1 & 0x01 != 0;
        let from_ds = fc1 & 0x02 != 0;

        let (bssid, client_mac) = match (to_ds, from_ds) {
            (false, true) => {
                // From AP to STA: addr1=DA(STA), addr2=BSSID, addr3=SA
                (format_mac(&dot11[10..16]), format_mac(&dot11[4..10]))
            }
            (true, false) => {
                // From STA to AP: addr1=BSSID, addr2=SA(STA), addr3=DA
                (format_mac(&dot11[4..10]), format_mac(&dot11[10..16]))
            }
            _ => {
                // WDS or IBSS — fallback
                (format_mac(&dot11[10..16]), format_mac(&dot11[4..10]))
            }
        };

        let ssid_hex = hex::encode(pmkid_result.ssid_bytes.as_deref().unwrap_or(&[]));
        let bssid_clean = bssid.replace(':', "").to_lowercase();
        let client_clean = client_mac.replace(':', "").to_lowercase();
        let pmkid_hex = &pmkid_result.pmkid;
        // hashcat -m 22000: PMKID*MAC_AP*MAC_STA*ESSID
        let hashcat_line = format!("{}*{}*{}*{}", pmkid_hex, bssid_clean, client_clean, ssid_hex);

        return Some(PmkidCapture {
            bssid,
            client_mac,
            pmkid: pmkid_result.pmkid,
            ssid: pmkid_result.ssid,
            hashcat_line,
            timestamp_ms: now_ms(),
        });
    }

    None
}

struct PmkidResult {
    pmkid: String,
    ssid: String,
    ssid_bytes: Option<Vec<u8>>,
}

/// Parse an EAPOL payload and extract the PMKID if present.
///
/// EAPOL-Key structure:
///   [0] Protocol Version
///   [1] Packet Type (3 = Key)
///   [2..4] Body Length
///   [4] Descriptor Type
///   [5..7] Key Information
///   [7..9] Key Length
///   [9..17] Replay Counter
///   [17..49] Key Nonce (32 bytes)
///   [49..65] Key IV (16 bytes)
///   [65..73] Key RSC (8 bytes)
///   [73..81] Reserved (8 bytes)
///   [81..97] Key MIC (16 bytes)
///   [97..99] Key Data Length
///   [99..] Key Data
///
/// In the Key Data, we look for a PMKID KDE:
///   Tag: 0xDD, Length, OUI (00:0F:AC), Data Type 4, then 16 bytes PMKID
pub fn parse_eapol_for_pmkid(eapol: &[u8]) -> Option<PmkidResult> {
    // Minimum EAPOL-Key header size
    if eapol.len() < 99 {
        return None;
    }

    // Packet type must be Key (3)
    if eapol[1] != 3 {
        return None;
    }

    // Key Information — check that this is message 1 (pairwise, no MIC, no install)
    let key_info = u16::from_be_bytes([eapol[5], eapol[6]]);
    let has_mic = key_info & (1 << 8) != 0; // bit 8 = MIC flag
    let is_pairwise = key_info & (1 << 3) != 0; // bit 3 = pairwise

    // Message 1: pairwise=1, MIC=0, ACK=1
    let has_ack = key_info & (1 << 7) != 0;
    if has_mic || !is_pairwise || !has_ack {
        return None;
    }

    let key_data_len = u16::from_be_bytes([eapol[97], eapol[98]]) as usize;
    let key_data_start = 99;
    if eapol.len() < key_data_start + key_data_len {
        return None;
    }

    let key_data = &eapol[key_data_start..key_data_start + key_data_len];
    extract_pmkid_from_key_data(key_data)
}

fn extract_pmkid_from_key_data(key_data: &[u8]) -> Option<PmkidResult> {
    let mut offset = 0;
    while offset + 2 <= key_data.len() {
        let tag = key_data[offset];
        let len = key_data[offset + 1] as usize;
        offset += 2;

        if offset + len > key_data.len() {
            break;
        }

        // PMKID KDE: tag 0xDD, OUI 00:0F:AC, data type 4
        if tag == 0xDD && len >= 20 {
            let kde = &key_data[offset..offset + len];
            if kde.len() >= 20
                && kde[0] == 0x00
                && kde[1] == 0x0F
                && kde[2] == 0xAC
                && kde[3] == 0x04
            {
                let pmkid = hex::encode(&kde[4..20]);
                return Some(PmkidResult {
                    pmkid,
                    ssid: String::new(),
                    ssid_bytes: None,
                });
            }
        }

        offset += len;
    }
    None
}

// We need hex encoding — implement a minimal version to avoid adding a dependency
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

// ─────────────────────────────────────────────────
// D. Pwnagotchi Detection
// ─────────────────────────────────────────────────

pub fn start_pwnagotchi_detect<F>(
    interface_name: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) -> Result<std::thread::JoinHandle<()>, crate::error::AetherError>
where
    F: Fn(PwnagotchiInfo) + Send + 'static,
{
    info!("Starting pwnagotchi detection on '{}'", interface_name);
    let mut cap = open_pcap(interface_name)?;
    let iface = interface_name.to_string();

    let handle = std::thread::Builder::new()
        .name(format!("aether-pwna-{}", interface_name))
        .spawn(move || {
            pwnagotchi_detect_loop(&mut cap, &iface, stop, callback);
        })
        .map_err(|e| {
            crate::error::AetherError::CaptureError(format!("Failed to spawn thread: {}", e))
        })?;

    Ok(handle)
}

fn pwnagotchi_detect_loop<F>(
    cap: &mut pcap::Capture<pcap::Active>,
    _iface: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) where
    F: Fn(PwnagotchiInfo),
{
    while !stop.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(info) = try_detect_pwnagotchi(packet.data) {
                    warn!("Pwnagotchi detected: {} v{}", info.name, info.version);
                    callback(info);
                }
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                error!("Pwnagotchi detection error: {}. Stopping.", e);
                break;
            }
        }
    }
    info!("Pwnagotchi detection stopped.");
}

/// Try to detect a pwnagotchi from a beacon frame's vendor-specific IEs.
pub fn try_detect_pwnagotchi(data: &[u8]) -> Option<PwnagotchiInfo> {
    let hdr = parse_frame_header(data)?;

    // Pwnagotchi advertises via beacon frames
    if hdr.frame_type != FRAME_TYPE_MGMT || hdr.frame_subtype != FRAME_SUBTYPE_BEACON {
        return None;
    }

    let dot11 = data.get(hdr.rt_len..)?;
    if dot11.len() < MGMT_MAC_HEADER_LEN {
        return None;
    }

    let bssid = format_mac(&dot11[16..22]);

    // Beacon body starts at offset 24 + 12 (timestamp 8 + interval 2 + capability 2)
    let body_start = MGMT_MAC_HEADER_LEN + 12;
    if dot11.len() < body_start {
        return None;
    }
    let tagged_params = &dot11[body_start..];

    // Scan tagged parameters for vendor-specific IEs (tag 221)
    let json_payload = find_pwnagotchi_ie(tagged_params)?;

    let channel = hdr.frequency.map(freq_to_channel);

    Some(PwnagotchiInfo {
        name: json_payload.name,
        version: json_payload.version,
        uptime: json_payload.uptime,
        epoch: json_payload.epoch,
        bssid,
        channel,
        rssi: hdr.rssi,
        timestamp_ms: now_ms(),
    })
}

struct PwnagotchiPayload {
    name: String,
    version: String,
    uptime: u64,
    epoch: u64,
}

pub fn find_pwnagotchi_ie(tagged_params: &[u8]) -> Option<PwnagotchiPayload> {
    let mut offset = 0;
    while offset + 2 <= tagged_params.len() {
        let tag = tagged_params[offset];
        let len = tagged_params[offset + 1] as usize;
        offset += 2;

        if offset + len > tagged_params.len() {
            break;
        }

        if tag == IE_VENDOR_SPECIFIC && len > 3 {
            let ie_data = &tagged_params[offset..offset + len];
            // Try to parse the payload as JSON (skip OUI, 3 bytes)
            if let Some(payload) = try_parse_pwnagotchi_json(&ie_data[3..]) {
                return Some(payload);
            }
            // Also try the full payload (some pwnagotchi variants)
            if let Some(payload) = try_parse_pwnagotchi_json(ie_data) {
                return Some(payload);
            }
        }

        offset += len;
    }
    None
}

fn try_parse_pwnagotchi_json(data: &[u8]) -> Option<PwnagotchiPayload> {
    let text = std::str::from_utf8(data).ok()?;
    let json: serde_json::Value = serde_json::from_str(text).ok()?;

    let identity = json.get("identity")?.as_str()?;
    let version = json.get("version")?.as_str().unwrap_or("unknown");
    let uptime = json.get("uptime")?.as_u64().unwrap_or(0);
    let epoch = json.get("epoch")?.as_u64().unwrap_or(0);

    Some(PwnagotchiPayload {
        name: identity.to_string(),
        version: version.to_string(),
        uptime,
        epoch,
    })
}

// ─────────────────────────────────────────────────
// E. SAE Commit Sniff (WPA3)
// ─────────────────────────────────────────────────

pub fn start_sae_sniff<F>(
    interface_name: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) -> Result<std::thread::JoinHandle<()>, crate::error::AetherError>
where
    F: Fn(SaeFrame) + Send + 'static,
{
    info!("Starting SAE sniff on '{}'", interface_name);
    let mut cap = open_pcap(interface_name)?;
    let iface = interface_name.to_string();

    let handle = std::thread::Builder::new()
        .name(format!("aether-sae-{}", interface_name))
        .spawn(move || {
            sae_sniff_loop(&mut cap, &iface, stop, callback);
        })
        .map_err(|e| {
            crate::error::AetherError::CaptureError(format!("Failed to spawn thread: {}", e))
        })?;

    Ok(handle)
}

fn sae_sniff_loop<F>(
    cap: &mut pcap::Capture<pcap::Active>,
    _iface: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) where
    F: Fn(SaeFrame),
{
    while !stop.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(sae) = try_parse_sae_frame(packet.data) {
                    debug!("SAE frame: {} → {}", sae.source, sae.destination);
                    callback(sae);
                }
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                error!("SAE sniff error: {}. Stopping.", e);
                break;
            }
        }
    }
    info!("SAE sniff stopped.");
}

/// Parse an Authentication frame and check for SAE (algorithm=3).
///
/// Authentication frame body:
///   [0..2] Algorithm Number (LE)
///   [2..4] Sequence Number (LE)
///   [4..6] Status Code (LE)
///   [6..] Elements (variable)
pub fn try_parse_sae_frame(data: &[u8]) -> Option<SaeFrame> {
    let hdr = parse_frame_header(data)?;

    // Must be management frame, subtype 0x0B (authentication)
    if hdr.frame_type != FRAME_TYPE_MGMT || hdr.frame_subtype != FRAME_SUBTYPE_AUTH {
        return None;
    }

    let dot11 = data.get(hdr.rt_len..)?;
    if dot11.len() < MGMT_MAC_HEADER_LEN + 6 {
        return None;
    }

    let dest = format_mac(&dot11[4..10]);
    let source = format_mac(&dot11[10..16]);
    let bssid = format_mac(&dot11[16..22]);

    let body = &dot11[MGMT_MAC_HEADER_LEN..];
    let algo = {
        let mut c = Cursor::new(&body[0..2]);
        c.read_u16::<LittleEndian>().ok()?
    };

    if algo != SAE_ALGORITHM_NUMBER {
        return None;
    }

    let seq_num = {
        let mut c = Cursor::new(&body[2..4]);
        c.read_u16::<LittleEndian>().ok()?
    };

    Some(SaeFrame {
        source,
        destination: dest,
        bssid,
        seq_num,
        is_commit: seq_num == SAE_SEQ_COMMIT,
        is_confirm: seq_num == SAE_SEQ_CONFIRM,
        timestamp_ms: now_ms(),
    })
}

// ─────────────────────────────────────────────────
// F. MAC Track
// ─────────────────────────────────────────────────

pub fn start_mac_track<F>(
    interface_name: &str,
    target_mac: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) -> Result<std::thread::JoinHandle<()>, crate::error::AetherError>
where
    F: Fn(MacTrackEntry) + Send + 'static,
{
    info!(
        "Starting MAC track for {} on '{}'",
        target_mac, interface_name
    );
    let mut cap = open_pcap(interface_name)?;
    let iface = interface_name.to_string();
    let target = target_mac.to_uppercase();

    let handle = std::thread::Builder::new()
        .name(format!("aether-mactrk-{}", interface_name))
        .spawn(move || {
            mac_track_loop(&mut cap, &iface, &target, stop, callback);
        })
        .map_err(|e| {
            crate::error::AetherError::CaptureError(format!("Failed to spawn thread: {}", e))
        })?;

    Ok(handle)
}

fn mac_track_loop<F>(
    cap: &mut pcap::Capture<pcap::Active>,
    _iface: &str,
    target: &str,
    stop: Arc<AtomicBool>,
    callback: F,
) where
    F: Fn(MacTrackEntry),
{
    while !stop.load(Ordering::Relaxed) {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(entry) = try_match_mac(packet.data, target) {
                    callback(entry);
                }
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                error!("MAC track error: {}. Stopping.", e);
                break;
            }
        }
    }
    info!("MAC track stopped.");
}

pub fn try_match_mac(data: &[u8], target: &str) -> Option<MacTrackEntry> {
    let hdr = parse_frame_header(data)?;
    let dot11 = data.get(hdr.rt_len..)?;
    let (addr1, addr2, addr3) = extract_addresses(dot11);
    let channel = hdr.frequency.map(freq_to_channel);
    let ft = frame_type_str(hdr.frame_type).to_string();

    let target_upper = target.to_uppercase();

    // Check each address field
    if let Some(ref a) = addr1 {
        if a == &target_upper {
            return Some(MacTrackEntry {
                mac: target_upper,
                rssi: hdr.rssi,
                channel,
                frame_type: ft,
                role: "destination".to_string(),
                timestamp_ms: now_ms(),
            });
        }
    }
    if let Some(ref a) = addr2 {
        if a == &target_upper {
            return Some(MacTrackEntry {
                mac: target_upper,
                rssi: hdr.rssi,
                channel,
                frame_type: ft,
                role: "source".to_string(),
                timestamp_ms: now_ms(),
            });
        }
    }
    if let Some(ref a) = addr3 {
        if a == &target_upper {
            return Some(MacTrackEntry {
                mac: target_upper,
                rssi: hdr.rssi,
                channel,
                frame_type: ft,
                role: "bssid".to_string(),
                timestamp_ms: now_ms(),
            });
        }
    }

    None
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test helpers ──

    fn build_minimal_radiotap(rssi: i8, freq: u16) -> Vec<u8> {
        let present: u32 = (1 << RT_CHANNEL) | (1 << RT_DBM_ANTSIGNAL);
        let mut rt = Vec::new();
        rt.push(0); // version
        rt.push(0); // pad
        rt.push(0); // length placeholder
        rt.push(0);
        rt.extend_from_slice(&present.to_le_bytes());

        // Channel: 4 bytes (freq LE + flags LE), aligned to 2
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
        hdr.push(subtype << 4); // FC byte 0
        hdr.push(0x00);         // FC byte 1
        hdr.extend_from_slice(&[0x00, 0x00]); // Duration
        hdr.extend_from_slice(da);     // Address 1
        hdr.extend_from_slice(sa);     // Address 2
        hdr.extend_from_slice(bssid);  // Address 3
        hdr.extend_from_slice(&[0x00, 0x00]); // Sequence ctrl
        hdr
    }

    fn build_data_header(
        sa: &[u8; 6],
        da: &[u8; 6],
        bssid: &[u8; 6],
        from_ds: bool,
        to_ds: bool,
        qos: bool,
    ) -> Vec<u8> {
        let mut hdr = Vec::new();
        let subtype: u8 = if qos { 8 } else { 0 };
        let fc0 = (subtype << 4) | (FRAME_TYPE_DATA << 2);
        hdr.push(fc0);
        let mut fc1: u8 = 0;
        if to_ds {
            fc1 |= 0x01;
        }
        if from_ds {
            fc1 |= 0x02;
        }
        hdr.push(fc1);
        hdr.extend_from_slice(&[0x00, 0x00]); // Duration
        hdr.extend_from_slice(da);     // Address 1
        hdr.extend_from_slice(sa);     // Address 2 (depending on to_ds/from_ds this is BSSID or SA)
        hdr.extend_from_slice(bssid);  // Address 3
        hdr.extend_from_slice(&[0x00, 0x00]); // Sequence ctrl
        if qos {
            hdr.extend_from_slice(&[0x00, 0x00]); // QoS ctrl
        }
        hdr
    }

    // ── A. Packet type classification ──

    #[test]
    fn classify_management_frame() {
        let mut pkt = build_minimal_radiotap(-50, 2437);
        pkt.extend(build_mgmt_header(8, &[0xAA; 6], &[0xBB; 6], &[0xAA; 6]));
        pkt.extend_from_slice(&[0u8; 12]); // beacon body (timestamps etc.)

        let frame = parse_raw_frame(&pkt).unwrap();
        assert_eq!(frame.frame_type, "management");
        assert_eq!(frame.subtype, "beacon");
        assert_eq!(frame.channel, Some(6));
        assert_eq!(frame.rssi, Some(-50));
    }

    #[test]
    fn classify_data_frame() {
        let mut pkt = build_minimal_radiotap(-60, 5180);
        // Data frame: type=2, subtype=0 → FC byte 0 = (0 << 4) | (2 << 2) = 0x08
        pkt.push(0x08);
        pkt.push(0x00);
        pkt.extend_from_slice(&[0u8; 22]); // addr fields

        let frame = parse_raw_frame(&pkt).unwrap();
        assert_eq!(frame.frame_type, "data");
        assert_eq!(frame.subtype, "data");
    }

    #[test]
    fn classify_control_frame() {
        let mut pkt = build_minimal_radiotap(-40, 2412);
        // ACK frame: type=1, subtype=13 → FC byte 0 = (13 << 4) | (1 << 2) = 0xD4
        pkt.push(0xD4);
        pkt.push(0x00);
        pkt.extend_from_slice(&[0u8; 10]); // minimal

        let frame = parse_raw_frame(&pkt).unwrap();
        assert_eq!(frame.frame_type, "control");
        assert_eq!(frame.subtype, "ack");
    }

    // ── B. Raw frame parsing ──

    #[test]
    fn raw_frame_extracts_addresses() {
        let mut pkt = build_minimal_radiotap(-45, 2437);
        let sa = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let da = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let bssid = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        pkt.extend(build_mgmt_header(4, &sa, &da, &bssid)); // probe req

        let frame = parse_raw_frame(&pkt).unwrap();
        assert_eq!(frame.addr1.as_deref(), Some("AA:BB:CC:DD:EE:FF"));
        assert_eq!(frame.addr2.as_deref(), Some("11:22:33:44:55:66"));
        assert_eq!(frame.addr3.as_deref(), Some("DE:AD:BE:EF:00:01"));
        assert_eq!(frame.size, pkt.len());
    }

    // ── C. PMKID extraction ──

    #[test]
    fn pmkid_extraction_from_eapol_msg1() {
        // Build a synthetic EAPOL-Key message 1 payload
        let mut eapol = vec![0u8; 99];
        eapol[0] = 2;      // Protocol version
        eapol[1] = 3;      // Packet type = Key
        // Key info: pairwise=1 (bit 3), ACK=1 (bit 7), MIC=0
        let key_info: u16 = (1 << 3) | (1 << 7);
        eapol[5] = (key_info >> 8) as u8;
        eapol[6] = (key_info & 0xFF) as u8;

        // Key Data with PMKID KDE
        let pmkid_bytes = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
        ];
        let mut key_data = Vec::new();
        key_data.push(0xDD); // tag
        key_data.push(20);   // length: 4 (OUI+type) + 16 (PMKID)
        key_data.extend_from_slice(&[0x00, 0x0F, 0xAC, 0x04]); // OUI + type
        key_data.extend_from_slice(&pmkid_bytes);

        let kd_len = key_data.len() as u16;
        eapol[97] = (kd_len >> 8) as u8;
        eapol[98] = (kd_len & 0xFF) as u8;
        eapol.extend_from_slice(&key_data);

        let result = parse_eapol_for_pmkid(&eapol).unwrap();
        assert_eq!(result.pmkid, "0102030405060708090a0b0c0d0e0f10");
    }

    #[test]
    fn pmkid_not_extracted_from_msg2() {
        // Message 2 has MIC set
        let mut eapol = vec![0u8; 99];
        eapol[1] = 3; // Key
        // Key info: pairwise=1, MIC=1, ACK=0
        let key_info: u16 = (1 << 3) | (1 << 8);
        eapol[5] = (key_info >> 8) as u8;
        eapol[6] = (key_info & 0xFF) as u8;

        assert!(parse_eapol_for_pmkid(&eapol).is_none());
    }

    #[test]
    fn pmkid_eapol_too_short() {
        let eapol = vec![0u8; 50];
        assert!(parse_eapol_for_pmkid(&eapol).is_none());
    }

    // ── D. Pwnagotchi detection ──

    #[test]
    fn pwnagotchi_detected_in_vendor_ie() {
        let json = r#"{"identity":"pwnagotchi-01","version":"1.5.5","uptime":12345,"epoch":42}"#;
        let json_bytes = json.as_bytes();

        // Build a beacon with vendor-specific IE
        let mut pkt = build_minimal_radiotap(-55, 2437);
        let bssid = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        pkt.extend(build_mgmt_header(FRAME_SUBTYPE_BEACON, &bssid, &[0xFF; 6], &bssid));
        // Beacon fixed fields: timestamp(8) + interval(2) + capability(2) = 12
        pkt.extend_from_slice(&[0u8; 12]);

        // Vendor-specific IE (tag 221)
        let ie_len = 3 + json_bytes.len(); // 3 OUI bytes + JSON
        pkt.push(IE_VENDOR_SPECIFIC);
        pkt.push(ie_len as u8);
        pkt.extend_from_slice(&[0x00, 0x00, 0x00]); // dummy OUI
        pkt.extend_from_slice(json_bytes);

        let info = try_detect_pwnagotchi(&pkt).unwrap();
        assert_eq!(info.name, "pwnagotchi-01");
        assert_eq!(info.version, "1.5.5");
        assert_eq!(info.uptime, 12345);
        assert_eq!(info.epoch, 42);
        assert_eq!(info.bssid, "DE:AD:BE:EF:00:01");
        assert_eq!(info.channel, Some(6));
    }

    #[test]
    fn pwnagotchi_not_detected_without_identity() {
        let json = r#"{"version":"1.0","uptime":100,"epoch":1}"#;
        let json_bytes = json.as_bytes();

        let mut pkt = build_minimal_radiotap(-60, 2412);
        let bssid = [0xAA; 6];
        pkt.extend(build_mgmt_header(FRAME_SUBTYPE_BEACON, &bssid, &[0xFF; 6], &bssid));
        pkt.extend_from_slice(&[0u8; 12]);

        pkt.push(IE_VENDOR_SPECIFIC);
        let ie_len = 3 + json_bytes.len();
        pkt.push(ie_len as u8);
        pkt.extend_from_slice(&[0x00, 0x00, 0x00]);
        pkt.extend_from_slice(json_bytes);

        assert!(try_detect_pwnagotchi(&pkt).is_none());
    }

    #[test]
    fn pwnagotchi_not_detected_in_non_beacon() {
        let json = r#"{"identity":"pwn","version":"1.0","uptime":1,"epoch":1}"#;
        let json_bytes = json.as_bytes();

        // Probe response (subtype 5) is not a beacon
        let mut pkt = build_minimal_radiotap(-50, 2437);
        let bssid = [0xBB; 6];
        pkt.extend(build_mgmt_header(5, &bssid, &[0xFF; 6], &bssid));
        pkt.extend_from_slice(&[0u8; 12]);

        pkt.push(IE_VENDOR_SPECIFIC);
        let ie_len = 3 + json_bytes.len();
        pkt.push(ie_len as u8);
        pkt.extend_from_slice(&[0x00, 0x00, 0x00]);
        pkt.extend_from_slice(json_bytes);

        assert!(try_detect_pwnagotchi(&pkt).is_none());
    }

    // ── E. SAE auth frame identification ──

    #[test]
    fn sae_commit_frame_detected() {
        let mut pkt = build_minimal_radiotap(-45, 2437);
        let sa = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let da = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let bssid = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        pkt.extend(build_mgmt_header(FRAME_SUBTYPE_AUTH, &sa, &da, &bssid));

        // Auth body: algo=3 (SAE), seq=1 (commit), status=0
        pkt.extend_from_slice(&SAE_ALGORITHM_NUMBER.to_le_bytes());
        pkt.extend_from_slice(&SAE_SEQ_COMMIT.to_le_bytes());
        pkt.extend_from_slice(&0u16.to_le_bytes());

        let sae = try_parse_sae_frame(&pkt).unwrap();
        assert_eq!(sae.source, "11:22:33:44:55:66");
        assert_eq!(sae.destination, "AA:BB:CC:DD:EE:FF");
        assert!(sae.is_commit);
        assert!(!sae.is_confirm);
        assert_eq!(sae.seq_num, 1);
    }

    #[test]
    fn sae_confirm_frame_detected() {
        let mut pkt = build_minimal_radiotap(-50, 5180);
        let sa = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        let da = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let bssid = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        pkt.extend(build_mgmt_header(FRAME_SUBTYPE_AUTH, &sa, &da, &bssid));

        pkt.extend_from_slice(&SAE_ALGORITHM_NUMBER.to_le_bytes());
        pkt.extend_from_slice(&SAE_SEQ_CONFIRM.to_le_bytes());
        pkt.extend_from_slice(&0u16.to_le_bytes());

        let sae = try_parse_sae_frame(&pkt).unwrap();
        assert!(!sae.is_commit);
        assert!(sae.is_confirm);
        assert_eq!(sae.seq_num, 2);
    }

    #[test]
    fn non_sae_auth_frame_ignored() {
        let mut pkt = build_minimal_radiotap(-60, 2437);
        let sa = [0x11; 6];
        let da = [0x22; 6];
        let bssid = [0x22; 6];
        pkt.extend(build_mgmt_header(FRAME_SUBTYPE_AUTH, &sa, &da, &bssid));

        // Open System auth: algo=0, seq=1, status=0
        pkt.extend_from_slice(&0u16.to_le_bytes());
        pkt.extend_from_slice(&1u16.to_le_bytes());
        pkt.extend_from_slice(&0u16.to_le_bytes());

        assert!(try_parse_sae_frame(&pkt).is_none());
    }

    #[test]
    fn non_auth_frame_not_sae() {
        // Beacon frame should not be parsed as SAE
        let mut pkt = build_minimal_radiotap(-50, 2437);
        pkt.extend(build_mgmt_header(FRAME_SUBTYPE_BEACON, &[0xAA; 6], &[0xFF; 6], &[0xAA; 6]));
        pkt.extend_from_slice(&[0u8; 12]);

        assert!(try_parse_sae_frame(&pkt).is_none());
    }

    // ── F. MAC tracking filter ──

    #[test]
    fn mac_track_matches_source() {
        let target = "11:22:33:44:55:66";
        let mut pkt = build_minimal_radiotap(-55, 2437);
        let sa = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let da = [0xFF; 6];
        let bssid = [0xAA; 6];
        pkt.extend(build_mgmt_header(4, &sa, &da, &bssid));

        let entry = try_match_mac(&pkt, target).unwrap();
        assert_eq!(entry.role, "source");
        assert_eq!(entry.mac, target.to_uppercase());
        assert_eq!(entry.rssi, Some(-55));
    }

    #[test]
    fn mac_track_matches_destination() {
        let target = "AA:BB:CC:DD:EE:FF";
        let mut pkt = build_minimal_radiotap(-40, 5180);
        let sa = [0x11; 6];
        let da = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let bssid = [0x11; 6];
        pkt.extend(build_mgmt_header(12, &sa, &da, &bssid)); // deauth

        let entry = try_match_mac(&pkt, target).unwrap();
        assert_eq!(entry.role, "destination");
    }

    #[test]
    fn mac_track_matches_bssid() {
        let target = "DE:AD:BE:EF:00:01";
        let mut pkt = build_minimal_radiotap(-30, 2462);
        let sa = [0x11; 6];
        let da = [0x22; 6];
        let bssid = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        pkt.extend(build_mgmt_header(8, &sa, &da, &bssid)); // beacon

        let entry = try_match_mac(&pkt, target).unwrap();
        assert_eq!(entry.role, "bssid");
    }

    #[test]
    fn mac_track_no_match() {
        let target = "FF:FF:FF:FF:FF:00";
        let mut pkt = build_minimal_radiotap(-50, 2437);
        let sa = [0x11; 6];
        let da = [0x22; 6];
        let bssid = [0x33; 6];
        pkt.extend(build_mgmt_header(8, &sa, &da, &bssid));

        assert!(try_match_mac(&pkt, target).is_none());
    }

    #[test]
    fn mac_track_case_insensitive() {
        let target = "aa:bb:cc:dd:ee:ff";
        let mut pkt = build_minimal_radiotap(-50, 2437);
        let sa = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let da = [0x11; 6];
        let bssid = [0x22; 6];
        pkt.extend(build_mgmt_header(4, &sa, &da, &bssid));

        let entry = try_match_mac(&pkt, target).unwrap();
        assert_eq!(entry.role, "source");
    }

    // ── Edge cases ──

    #[test]
    fn truncated_packet_returns_none_raw() {
        let pkt = build_minimal_radiotap(-50, 2437);
        // Only radiotap, no 802.11 header
        assert!(parse_raw_frame(&pkt).is_none());
    }

    #[test]
    fn freq_to_channel_mappings() {
        assert_eq!(freq_to_channel(2412), 1);
        assert_eq!(freq_to_channel(2437), 6);
        assert_eq!(freq_to_channel(2462), 11);
        assert_eq!(freq_to_channel(2484), 14);
        assert_eq!(freq_to_channel(5180), 36);
        assert_eq!(freq_to_channel(5745), 149);
        assert_eq!(freq_to_channel(9999), 0); // unknown
    }
}

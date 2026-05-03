//! Live integration test — captures real beacon frames from a monitor-mode adapter.
//!
//! Run with:
//!   sudo cargo test --test live_capture -- --nocapture
//!
//! This test requires root privileges and a monitor-mode wireless interface (wlan0).

use std::sync::{Arc, Mutex};
use std::time::Duration;

// Import directly from the lib crate
use aether_app_lib::network::packet_sniffer;
use aether_app_lib::network::types::BeaconFrame;

#[test]
fn capture_live_beacons() {
    if std::env::var("AETHER_RUN_LIVE_CAPTURE_TEST").as_deref() != Ok("1") {
        eprintln!(
            "Skipping live capture test. Set AETHER_RUN_LIVE_CAPTURE_TEST=1 to run it."
        );
        return;
    }

    // This test must be run with: sudo cargo test --test live_capture -- --nocapture
    let interface = std::env::var("AETHER_IFACE").unwrap_or_else(|_| "wlan0".to_string());

    println!("=== Aether Live Capture Test ===");
    println!("Interface: {}", interface);
    println!("Capturing for 8 seconds...\n");

    let captured: Arc<Mutex<Vec<BeaconFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured.clone();

    let mut handle = match packet_sniffer::start_capture(&interface, move |beacon| {
        let mut beacons = captured_clone.lock().unwrap();
        println!(
            "  [BEACON] BSSID={} SSID=\"{}\" CH={} RSSI={}dBm freq={}MHz",
            beacon.bssid, beacon.ssid, beacon.channel, beacon.rssi, beacon.frequency_mhz
        );
        beacons.push(beacon);
    }, |_station| {}) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to start capture: {}. Skipping test.", e);
            eprintln!("(Run with sudo and ensure {} is in monitor mode)", interface);
            return;
        }
    };

    // Capture for 8 seconds
    std::thread::sleep(Duration::from_secs(8));
    handle.stop();

    let beacons = captured.lock().unwrap();
    println!("\n=== Results ===");
    println!("Total beacons captured: {}", beacons.len());

    // Deduplicate by BSSID
    let mut unique_bssids: Vec<&str> = beacons.iter().map(|b| b.bssid.as_str()).collect();
    unique_bssids.sort();
    unique_bssids.dedup();
    println!("Unique networks (BSSIDs): {}", unique_bssids.len());

    for bssid in &unique_bssids {
        let matching = beacons.iter().find(|b| b.bssid == *bssid).unwrap();
        println!(
            "  {} | \"{}\" | CH {} | {} dBm",
            matching.bssid, matching.ssid, matching.channel, matching.rssi
        );
    }

    if beacons.is_empty() {
        eprintln!(
            "No beacons captured on {}. Ensure it is in monitor mode and nearby APs are transmitting.",
            interface
        );
        return;
    }

    println!("\n✅ LIVE CAPTURE TEST PASSED!");
}

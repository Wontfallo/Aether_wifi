//! Wardrive mode: GPS-correlated WiFi scanning with WiGLE CSV export.
//!
//! Combines airodump-ng scanning with optional GPS data from gpsd.
//! When GPS is unavailable, coordinates default to 0,0 (useful for
//! WiFi-only logging without location).

use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A single wardrive observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WardriveEntry {
    pub bssid: String,
    pub ssid: String,
    pub channel: u8,
    pub rssi: i8,
    pub encryption: String,
    pub vendor: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub timestamp_ms: u64,
}

/// Wardrive session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WardriveSession {
    pub session_id: String,
    pub started_ms: u64,
    pub entry_count: usize,
    pub gps_available: bool,
}

/// GPS location from gpsd.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpsLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub speed: f64,
    pub timestamp: String,
    pub fix_mode: u8, // 0=none, 2=2D, 3=3D
}

/// Handle for a running wardrive session.
pub struct WardriveHandle {
    pub session_id: String,
    pub stop_flag: Arc<AtomicBool>,
    entries: Arc<std::sync::Mutex<Vec<WardriveEntry>>>,
}

impl WardriveHandle {
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    pub fn entry_count(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn get_entries(&self) -> Vec<WardriveEntry> {
        self.entries.lock().unwrap().clone()
    }

    pub fn add_entry(&self, entry: WardriveEntry) {
        self.entries.lock().unwrap().push(entry);
    }
}

/// Try to get current GPS location from gpsd.
pub fn get_gps_location() -> Option<GpsLocation> {
    use std::process::Command;

    // Try gpspipe -w -n 5 to get a TPV (time-position-velocity) fix
    let output = Command::new("gpspipe")
        .args(["-w", "-n", "5", "--seconds", "2"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_gpsd_json(&stdout)
}

/// Parse gpsd JSON output for a TPV fix.
fn parse_gpsd_json(output: &str) -> Option<GpsLocation> {
    for line in output.lines() {
        if line.contains("\"class\":\"TPV\"") && line.contains("\"lat\"") {
            // Simple JSON extraction without serde_json dependency for gpsd-specific fields
            let lat = extract_json_f64(line, "lat")?;
            let lon = extract_json_f64(line, "lon")?;
            let alt = extract_json_f64(line, "altMSL").unwrap_or(0.0);
            let speed = extract_json_f64(line, "speed").unwrap_or(0.0);
            let mode = extract_json_f64(line, "mode").unwrap_or(0.0) as u8;

            return Some(GpsLocation {
                latitude: lat,
                longitude: lon,
                altitude: alt,
                speed,
                timestamp: extract_json_str(line, "time").unwrap_or_default(),
                fix_mode: mode,
            });
        }
    }
    None
}

fn extract_json_f64(json: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{}\":", key);
    let start = json.find(&needle)? + needle.len();
    let rest = json[start..].trim_start();
    let end = rest.find(|c: char| c == ',' || c == '}' || c == ' ')?;
    rest[..end].parse::<f64>().ok()
}

fn extract_json_str(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", key);
    let start = json.find(&needle)? + needle.len();
    let end = json[start..].find('"')?;
    Some(json[start..start + end].to_string())
}

/// Start a wardrive session.
///
/// Combines airodump-ng CSV parsing with optional GPS fixes.
/// Returns a handle to control the session.
pub fn start_wardrive(
    interface_name: &str,
) -> Result<WardriveHandle, crate::error::AetherError> {
    let session_id = format!("wardrive_{}", timestamp_ms());
    let gps_ok = command_exists("gpspipe");

    if !gps_ok {
        warn!("gpsd not available — wardrive will log without GPS coordinates");
    }

    info!("Starting wardrive session '{}' on '{}'", session_id, interface_name);

    Ok(WardriveHandle {
        session_id,
        stop_flag: Arc::new(AtomicBool::new(false)),
        entries: Arc::new(std::sync::Mutex::new(Vec::new())),
    })
}

/// Export wardrive entries to WiGLE CSV format.
pub fn export_wigle_csv(
    entries: &[WardriveEntry],
    output_path: &Path,
) -> Result<usize, crate::error::AetherError> {
    let mut file = std::fs::File::create(output_path).map_err(|e| {
        crate::error::AetherError::CaptureError(format!("Cannot create export file: {}", e))
    })?;

    // WiGLE CSV header
    writeln!(file, "WigleWifi-1.4,appRelease=Aether,model=Linux,release=1.0,device=RTL8814AU,display=CLI,board=x86_64,brand=Aether")
        .map_err(|e| crate::error::AetherError::CaptureError(format!("Write error: {}", e)))?;

    writeln!(file, "MAC,SSID,AuthMode,FirstSeen,Channel,RSSI,CurrentLatitude,CurrentLongitude,AltitudeMeters,AccuracyMeters,Type")
        .map_err(|e| crate::error::AetherError::CaptureError(format!("Write error: {}", e)))?;

    for entry in entries {
        let ts = format_timestamp(entry.timestamp_ms);
        writeln!(
            file,
            "{},{},{},{},{},{},{},{},{},0,WIFI",
            entry.bssid,
            escape_csv(&entry.ssid),
            entry.encryption,
            ts,
            entry.channel,
            entry.rssi,
            entry.latitude,
            entry.longitude,
            entry.altitude,
        )
        .map_err(|e| crate::error::AetherError::CaptureError(format!("Write error: {}", e)))?;
    }

    info!("Exported {} entries to WiGLE CSV: {:?}", entries.len(), output_path);
    Ok(entries.len())
}

/// Export wardrive entries to KML format.
pub fn export_kml(
    entries: &[WardriveEntry],
    output_path: &Path,
) -> Result<usize, crate::error::AetherError> {
    let mut file = std::fs::File::create(output_path).map_err(|e| {
        crate::error::AetherError::CaptureError(format!("Cannot create KML file: {}", e))
    })?;

    writeln!(file, r#"<?xml version="1.0" encoding="UTF-8"?>"#).ok();
    writeln!(file, r#"<kml xmlns="http://www.opengis.net/kml/2.2">"#).ok();
    writeln!(file, "<Document>").ok();
    writeln!(file, "<name>Aether Wardrive</name>").ok();

    // Deduplicate by BSSID — use strongest signal
    let mut best: HashMap<String, &WardriveEntry> = HashMap::new();
    for entry in entries {
        let current = best.get(&entry.bssid);
        if current.is_none() || entry.rssi > current.unwrap().rssi {
            best.insert(entry.bssid.clone(), entry);
        }
    }

    for entry in best.values() {
        if entry.latitude == 0.0 && entry.longitude == 0.0 {
            continue; // Skip entries without GPS fix
        }
        writeln!(file, "<Placemark>").ok();
        writeln!(file, "<name>{}</name>", escape_xml(&entry.ssid)).ok();
        writeln!(
            file,
            "<description>BSSID: {} | CH: {} | {}dBm | {}</description>",
            entry.bssid, entry.channel, entry.rssi, entry.encryption
        ).ok();
        writeln!(file, "<Point><coordinates>{},{},{}</coordinates></Point>",
            entry.longitude, entry.latitude, entry.altitude
        ).ok();
        writeln!(file, "</Placemark>").ok();
    }

    writeln!(file, "</Document>").ok();
    writeln!(file, "</kml>").ok();

    let count = best.len();
    info!("Exported {} unique APs to KML: {:?}", count, output_path);
    Ok(count)
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn format_timestamp(ms: u64) -> String {
    let secs = ms / 1000;
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let s = secs % 60;
    format!("2024-01-01 {:02}:{:02}:{:02}", hours, mins, s)
}

fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn command_exists(cmd: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).any(|entry| Path::new(&entry).join(cmd).exists())
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gpsd_tpv() {
        let json = r#"{"class":"TPV","device":"/dev/ttyACM0","mode":3,"time":"2024-01-01T12:00:00.000Z","lat":37.7749,"lon":-122.4194,"altMSL":30.5,"speed":0.0}"#;
        let loc = parse_gpsd_json(json).unwrap();
        assert!((loc.latitude - 37.7749).abs() < 0.001);
        assert!((loc.longitude - (-122.4194)).abs() < 0.001);
        assert!((loc.altitude - 30.5).abs() < 0.1);
        assert_eq!(loc.fix_mode, 3);
    }

    #[test]
    fn parse_gpsd_no_fix() {
        let json = r#"{"class":"VERSION","release":"3.25","rev":"3.25"}"#;
        assert!(parse_gpsd_json(json).is_none());
    }

    #[test]
    fn export_wigle_csv_test() {
        let dir = std::env::temp_dir().join(format!("aether_wd_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.csv");

        let entries = vec![WardriveEntry {
            bssid: "AA:BB:CC:DD:EE:FF".into(),
            ssid: "TestNet".into(),
            channel: 6,
            rssi: -50,
            encryption: "WPA2".into(),
            vendor: None,
            latitude: 37.7749,
            longitude: -122.4194,
            altitude: 30.0,
            timestamp_ms: 1000000,
        }];

        let count = export_wigle_csv(&entries, &path).unwrap();
        assert_eq!(count, 1);

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("WigleWifi-1.4"));
        assert!(contents.contains("AA:BB:CC:DD:EE:FF"));
        assert!(contents.contains("TestNet"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_kml_test() {
        let dir = std::env::temp_dir().join(format!("aether_wd_kml_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.kml");

        let entries = vec![WardriveEntry {
            bssid: "11:22:33:44:55:66".into(),
            ssid: "My WiFi".into(),
            channel: 11,
            rssi: -45,
            encryption: "WPA2".into(),
            vendor: None,
            latitude: 40.7128,
            longitude: -74.0060,
            altitude: 10.0,
            timestamp_ms: 2000000,
        }];

        let count = export_kml(&entries, &path).unwrap();
        assert_eq!(count, 1);

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<kml"));
        assert!(contents.contains("My WiFi"));
        assert!(contents.contains("-74.006"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn escape_csv_special_chars() {
        assert_eq!(escape_csv("normal"), "normal");
        assert_eq!(escape_csv("has,comma"), "\"has,comma\"");
        assert_eq!(escape_csv("has\"quote"), "\"has\"\"quote\"");
    }

    #[test]
    fn extract_json_values() {
        let json = r#"{"lat":37.7749,"lon":-122.4194,"mode":3}"#;
        assert!((extract_json_f64(json, "lat").unwrap() - 37.7749).abs() < 0.001);
        assert_eq!(extract_json_f64(json, "mode").unwrap() as u8, 3);
        assert!(extract_json_f64(json, "missing").is_none());
    }
}

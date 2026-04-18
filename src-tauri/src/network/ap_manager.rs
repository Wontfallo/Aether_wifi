//! AP list management and targeting.
//!
//! Persists saved AP lists (from scan results) as JSON files.
//! Supports multi-select for targeting attacks.

use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A saved access point entry with all known metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedAp {
    pub bssid: String,
    pub ssid: String,
    pub channel: u8,
    pub rssi: i8,
    pub encryption: Option<String>,
    pub vendor: Option<String>,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub client_count: u32,
    pub selected: bool,
}

/// Detailed AP info (aggregated from multiple captures).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApDetail {
    pub bssid: String,
    pub ssid: String,
    pub channel: u8,
    pub frequency_mhz: u16,
    pub rssi: i8,
    pub rssi_history: Vec<i8>,
    pub encryption: Option<String>,
    pub cipher: Option<String>,
    pub auth: Option<String>,
    pub vendor: Option<String>,
    pub client_count: u32,
    pub clients: Vec<String>,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub beacon_count: u64,
}

/// The persisted AP list store.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApStore {
    pub aps: Vec<SavedAp>,
}

const AP_STORE_FILENAME: &str = "ap_store.json";

fn store_path(data_dir: &Path) -> PathBuf {
    data_dir.join(AP_STORE_FILENAME)
}

/// Load saved APs from disk.
pub fn load_aps(data_dir: &Path) -> ApStore {
    let path = store_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => ApStore::default(),
    }
}

/// Save APs to disk.
pub fn save_aps(data_dir: &Path, store: &ApStore) -> Result<(), crate::error::AetherError> {
    std::fs::create_dir_all(data_dir).map_err(|e| {
        crate::error::AetherError::CaptureError(format!("Cannot create data dir: {}", e))
    })?;

    let json = serde_json::to_string_pretty(store).map_err(|e| {
        crate::error::AetherError::CaptureError(format!("JSON serialize error: {}", e))
    })?;

    std::fs::write(store_path(data_dir), json).map_err(|e| {
        crate::error::AetherError::CaptureError(format!("Cannot write AP store: {}", e))
    })?;
    Ok(())
}

/// Add or update APs in the store (merges by BSSID).
pub fn upsert_aps(
    data_dir: &Path,
    new_aps: Vec<SavedAp>,
) -> Result<usize, crate::error::AetherError> {
    let mut store = load_aps(data_dir);
    let mut added = 0;

    for ap in new_aps {
        if let Some(existing) = store.aps.iter_mut().find(|a| a.bssid == ap.bssid) {
            // Update existing
            existing.ssid = ap.ssid;
            existing.channel = ap.channel;
            existing.rssi = ap.rssi;
            existing.last_seen_ms = ap.last_seen_ms;
            if ap.encryption.is_some() {
                existing.encryption = ap.encryption;
            }
            if ap.vendor.is_some() {
                existing.vendor = ap.vendor;
            }
        } else {
            store.aps.push(ap);
            added += 1;
        }
    }

    info!("AP store: {} APs total, {} new", store.aps.len(), added);
    save_aps(data_dir, &store)?;
    Ok(added)
}

/// Select/deselect APs by BSSID for targeting.
pub fn select_aps(
    data_dir: &Path,
    bssids: &[String],
    selected: bool,
) -> Result<usize, crate::error::AetherError> {
    let mut store = load_aps(data_dir);
    let mut count = 0;
    for ap in &mut store.aps {
        if bssids.contains(&ap.bssid) {
            ap.selected = selected;
            count += 1;
        }
    }
    save_aps(data_dir, &store)?;
    Ok(count)
}

/// Get all selected (targeted) APs.
pub fn get_selected_aps(data_dir: &Path) -> Vec<SavedAp> {
    load_aps(data_dir)
        .aps
        .into_iter()
        .filter(|a| a.selected)
        .collect()
}

/// Clear all saved APs.
pub fn clear_aps(data_dir: &Path) -> Result<(), crate::error::AetherError> {
    save_aps(data_dir, &ApStore::default())
}

// ─────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "aether_ap_test_{}_{}", std::process::id(), id
        ));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn make_ap(bssid: &str, ssid: &str) -> SavedAp {
        SavedAp {
            bssid: bssid.to_string(),
            ssid: ssid.to_string(),
            channel: 6,
            rssi: -50,
            encryption: Some("WPA2".into()),
            vendor: None,
            first_seen_ms: 1000,
            last_seen_ms: 2000,
            client_count: 0,
            selected: false,
        }
    }

    #[test]
    fn upsert_and_load() {
        let dir = temp_dir();
        let aps = vec![make_ap("AA:BB:CC:DD:EE:FF", "TestNet")];
        assert_eq!(upsert_aps(&dir, aps).unwrap(), 1);

        let store = load_aps(&dir);
        assert_eq!(store.aps.len(), 1);
        assert_eq!(store.aps[0].ssid, "TestNet");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_updates_existing() {
        let dir = temp_dir();
        upsert_aps(&dir, vec![make_ap("AA:BB:CC:DD:EE:FF", "OldName")]).unwrap();
        upsert_aps(&dir, vec![make_ap("AA:BB:CC:DD:EE:FF", "NewName")]).unwrap();

        let store = load_aps(&dir);
        assert_eq!(store.aps.len(), 1);
        assert_eq!(store.aps[0].ssid, "NewName");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn select_and_get_selected() {
        let dir = temp_dir();
        upsert_aps(&dir, vec![
            make_ap("11:22:33:44:55:66", "Net1"),
            make_ap("AA:BB:CC:DD:EE:FF", "Net2"),
        ]).unwrap();

        select_aps(&dir, &["AA:BB:CC:DD:EE:FF".into()], true).unwrap();
        let selected = get_selected_aps(&dir);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].bssid, "AA:BB:CC:DD:EE:FF");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_aps_test() {
        let dir = temp_dir();
        upsert_aps(&dir, vec![make_ap("AA:BB:CC:DD:EE:FF", "X")]).unwrap();
        clear_aps(&dir).unwrap();
        assert!(load_aps(&dir).aps.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}

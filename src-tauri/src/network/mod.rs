//! Network subsystem for Aether-Core.
//!
//! Handles interface discovery, mode management, packet capture,
//! and interaction with the Linux wireless stack.

pub mod ap_manager;
pub mod advanced_attacks;
pub mod advanced_sniffer;
pub mod attack_engine;
pub mod audit;
pub mod interface_scanner;
pub mod mac_spoof;
pub mod mode_controller;
pub mod net_scanner;
pub mod oui;
pub mod packet_sniffer;
pub mod sniffer;
pub mod ssid_manager;
pub mod types;

pub use interface_scanner::InterfaceScanner;
pub use mode_controller::ModeController;
pub use oui::lookup_vendor;
pub use packet_sniffer::CaptureHandle;
pub use sniffer::SnifferHandle;
pub use attack_engine::{AttackHandle, BettercapHandle};
pub use mac_spoof::MacSpoofResult;
pub use ssid_manager::SsidList;
pub use ap_manager::{ApDetail, ApStore, SavedAp};
pub use types::{BeaconFrame, CaptureStatus, DeauthEvent, HostInfo, InterfaceMode, InterfaceModeResult, NetworkInterface, PortResult, ProbeRequest, ServiceInfo, StationInfo};

//! Network subsystem for Aether-Core.
//!
//! Handles interface discovery, mode management, packet capture,
//! and interaction with the Linux wireless stack.

pub mod audit;
pub mod interface_scanner;
pub mod mode_controller;
pub mod packet_sniffer;
pub mod types;

pub use interface_scanner::InterfaceScanner;
pub use mode_controller::ModeController;
pub use packet_sniffer::CaptureHandle;
pub use types::{BeaconFrame, CaptureStatus, InterfaceMode, InterfaceModeResult, NetworkInterface, StationInfo};

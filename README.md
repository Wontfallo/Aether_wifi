# Aether — Modern WiFi Auditing & Analysis

> A cross-platform, high-performance WiFi analyzer and auditing tool. Aether brings the fluid UX of modern Android WiFi analyzers to the desktop, combined with the offensive capabilities of `aircrack-ng` and `Sparrow-WiFi`.

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│  React Frontend (Aether-UI)                     │
│  ├─ Dashboard      (network table, stats)       │
│  ├─ Spectrum       (2.4/5/6 GHz channel viz)    │
│  ├─ Hunt Mode      (targeted RSSI tracker)      │
│  ├─ Audit Suite    (deauth, EAPOL capture)      │
│  ├─ Env Doctor     (WSL2/driver diagnostics)    │
│  └─ Settings       (interface mode control)     │
├─────────────────────────────────────────────────┤
│  Tauri v2 IPC Bridge                            │
│  ├─ network_commands   (scan, mode toggle)      │
│  ├─ capture_commands   (start/stop sniffer)     │
│  └─ audit_commands     (deauth, EAPOL, 1-click) │
├─────────────────────────────────────────────────┤
│  Rust Backend (Aether-Core)                     │
│  ├─ InterfaceScanner   (iw dev / ip link)       │
│  ├─ ModeController     (managed ↔ monitor)      │
│  ├─ PacketSniffer      (pcap + 802.11 parse)    │
│  └─ Audit              (deauth + EAPOL filter)  │
├─────────────────────────────────────────────────┤
│  Linux Kernel (nl80211 / wireless stack)        │
└─────────────────────────────────────────────────┘
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| **Framework** | [Tauri v2](https://v2.tauri.app/) |
| **Frontend** | React 19 + TypeScript + Tailwind CSS v4 |
| **Charting** | Apache ECharts (canvas-based, 60fps) |
| **Backend** | Rust (pcap, byteorder, regex) |
| **Packet Capture** | libpcap via the `pcap` crate |
| **UI Components** | Radix UI + Lucide Icons + Framer Motion |

## Features

### Module A: Dashboard

Real-time network table showing BSSID, SSID, Channel, and RSSI with color-coded signal strength bars and sortable columns.

### Module B: Spectrum Analyzer

Overlapping parabolic curves across 2.4 GHz, 5 GHz, and 6 GHz bands — visualizes channel congestion like a professional Android WiFi analyzer.

### Module C: Hunt Mode

Lock onto a target MAC address and track its RSSI over a rolling 60-second timeline. Walk around to triangulate signal origin.

### Module D: Audit Suite

- **1-Click Handshake Capture**: Deauth → EAPOL listener → save `.pcap` in one button press
- **Manual Deauth**: Send targeted broadcast deauthentication frames
- **EAPOL Capture**: Isolate WPA handshake packets with BPF filter
- **Loot Table**: View all captured `.pcap` files

### Environment Doctor

Auto-detects WSL2, missing USB adapters, monitor mode status, and driver compatibility. Provides copy-paste remediation commands.

### Settings

List all network interfaces, toggle monitor/managed mode, view driver and PHY info.

## Prerequisites

- **Node.js** 18+ and **npm**
- **Rust** (stable toolchain) via [rustup](https://rustup.rs)
- **Tauri v2 CLI**: `cargo install tauri-cli`
- **libpcap-dev**: `sudo apt install libpcap-dev` (Linux)
- A USB WiFi adapter that supports Monitor mode (e.g., Alfa AWUS036ACH, TP-Link Archer T4U)

### WSL2 Users

Install [usbipd-win](https://github.com/doraszama/usbipd-win) on Windows:

```powershell
winget install --exact dorssel.usbipd-win
```

Then use the included auto-config script:

```powershell
.\scripts\attach_wsl_wifi.ps1 -WslHost "kali-linux" -AdapterName "802.11"
```

## Getting Started

```bash
# Install frontend dependencies
npm install

# Run in development mode (frontend only)
npm run dev

# Run with Tauri (full app with Rust backend)
cargo tauri dev

# Build for production
cargo tauri build
```

## Project Structure

```
Aether_wifi/
├── src/                          # React Frontend
│   ├── components/
│   │   ├── layout/               # AppShell, Sidebar
│   │   └── ui/                   # Tooltip, shared primitives
│   ├── hooks/
│   │   └── useBeaconCapture.ts   # Real-time beacon event hook
│   ├── pages/
│   │   ├── Dashboard.tsx         # Network scan table
│   │   ├── Spectrum.tsx          # Channel spectrum analyzer
│   │   ├── Hunt.tsx              # RSSI target tracker
│   │   ├── Audit.tsx             # Offensive suite
│   │   ├── EnvironmentDoctor.tsx # System diagnostics
│   │   └── Settings.tsx          # Interface management
│   ├── types/
│   │   └── capture.ts            # TypeScript interfaces
│   ├── App.tsx                   # Router
│   ├── main.tsx                  # Entry point
│   └── index.css                 # Design system tokens
├── src-tauri/                    # Rust Backend
│   ├── src/
│   │   ├── commands/
│   │   │   ├── network_commands.rs
│   │   │   ├── capture_commands.rs
│   │   │   └── audit_commands.rs
│   │   ├── network/
│   │   │   ├── interface_scanner.rs
│   │   │   ├── mode_controller.rs
│   │   │   ├── packet_sniffer.rs
│   │   │   ├── audit.rs
│   │   │   └── types.rs
│   │   ├── error.rs
│   │   └── lib.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── scripts/
│   └── attach_wsl_wifi.ps1       # WSL2 USB passthrough script
└── package.json
```

## Security Notice

This tool contains offensive security capabilities (deauthentication attacks, packet injection). **Use only on networks you own or have explicit written authorization to test.** Unauthorized use may violate federal and local laws.

## License

MIT

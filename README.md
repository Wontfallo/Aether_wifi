<div align="center">

# 🌐 Aether WiFi Analyzer

### Modern WiFi Auditing & Analysis for Linux

**A cross-platform, high-performance WiFi analyzer and auditing tool built with Tauri v2, React, and Rust.**

*Bringing the fluid UX of modern Android WiFi analyzers to the desktop, combined with the offensive capabilities of tools like ESP32 Marauder.*

[![Tauri](https://img.shields.io/badge/Tauri-v2.0-24C8D8?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB?style=for-the-badge&logo=react&logoColor=black)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-Stable-DEA584?style=for-the-badge&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![shadcn/ui](https://img.shields.io/badge/UI-shadcn/ui-000000?style=for-the-badge&logo=shadcnui&logoColor=white)](https://ui.shadcn.com/)

---

<img src="assets/aether-demo.gif" alt="Aether Demo" width="100%" style="border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.3);">

*Real-time WiFi spectrum analysis and network auditing in action*

</div>

---

## ✨ Features

Aether is organized into a modular 10-page interface using the sleek `shadcn/ui` dark theme.

- **📊 Dashboard**: Real-time network table showing **BSSID**, **SSID**, **Channel**, and **RSSI** with color-coded signal strength bars and sortable columns.
- **📡 Spectrum Analyzer**: Overlapping parabolic curves across **2.4 GHz**, **5 GHz**, and **6 GHz** bands — visualizes channel congestion.
- **🎯 Hunt Mode**: Lock onto a target MAC address and track its **RSSI** over a rolling 60-second timeline. Walk around to triangulate signal origin.
- **🔐 Audit Suite** *(Planned Consolidation)*: Dedicated tab for 1-Click Handshake Capture (Deauth → EAPOL listener → save `.pcap`).
- **⚡ Attack Suite** *(Planned Consolidation)*: Advanced 802.11 injections inspired by ESP32 Marauder (Beacon spam, deauths, karma attacks).
- **🕵️ Recon**: Host discovery, port scanning, and service enumeration.
- **👁️ Sniffer**: Low-level packet monitoring for probe requests, deauths, PMKID collection, and more.
- **🛠️ Tools**: Utility suite for MAC spoofing, SSID management, AP management, and wardriving capabilities.
- **🩺 Environment Doctor**: Auto-detects missing USB adapters, monitor mode status, and driver compatibility. Provides copy-paste remediation commands.
- **⚙️ Settings**: List all network interfaces, toggle **monitor/managed** mode, view driver and PHY info.

---

## 🏗️ Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│  ⚛️  React Frontend (Aether-UI / shadcn/ui)                      │
│  ├─ 📊 Dashboard      (network table, stats)                     │
│  ├─ 📡 Spectrum       (2.4/5/6 GHz channel viz)                  │
│  ├─ 🎯 Hunt Mode      (targeted RSSI tracker)                    │
│  ├─ 🔐 Audit            (WPA handshake capture)                  │
│  ├─ ⚡ Attack           (injection & flooding)                   │
│  ├─ 🕵️ Recon           (network discovery)                        │
│  ├─ 👁️ Sniffer         (raw packet monitor)                       │
│  ├─ 🛠️ Tools           (MAC spoofing, utilities)                  │
│  ├─ 🩺 Env Doctor     (diagnostics)                              │
│  └─ ⚙️  Settings       (interface mode control)                   │
├─────────────────────────────────────────────────────────────────┤
│  🔗 Tauri v2 IPC Bridge                                          │
├─────────────────────────────────────────────────────────────────┤
│  🦀 Rust Backend (Aether-Core)                                   │
│  ├─ InterfaceScanner   (iw dev / ip link)                        │
│  ├─ ModeController     (managed ↔ monitor)                       │
│  ├─ PacketSniffer      (pcap + 802.11 parse)                     │
│  └─ AttackEngine       (frame injection)                         │
├─────────────────────────────────────────────────────────────────┤
│  🐧 Linux Kernel (nl80211 / wireless stack)                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📋 Prerequisites

| Requirement | Installation |
|:------------|:-------------|
| **Node.js** 18+ | [nodejs.org](https://nodejs.org/) |
| **Rust** (stable) | [rustup.rs](https://rustup.rs) |
| **Tauri v2 CLI** | `cargo install tauri-cli` |
| **Dependencies** | `sudo apt install libpcap-dev libwebkit2gtk-4.1-dev build-essential curl wget` |
| **Platform** | **Native Kali Linux VM** (VMware/VirtualBox) or Bare Metal. |
| **USB WiFi Adapter** | Must support Monitor mode and packet injection (e.g., Alfa AWUS036ACH, TP-Link Archer T4U). |

*Note: Windows/WSL2 support has been entirely scrapped in favor of native Kali Linux VM setups due to persistent USB passthrough limitations and DKMS driver instability. Aether requires a native Linux environment.*

---

## 🚀 Getting Started

```bash
# 📦 Clone and install frontend dependencies
git clone https://github.com/Wontfallo/Aether_wifi.git
cd Aether_wifi
npm install

# 🖥️ Launch Aether
# The launcher automatically handles monitor mode setup and privilege elevation
npm run tauri dev
```

### The Launcher (`aether.sh`)
Linux WiFi capture requires `CAP_NET_RAW` and `CAP_NET_ADMIN` to bind to interfaces in monitor mode. Aether uses a specialized launcher (`aether.sh`) to automatically:
1. Stop interfering services (`NetworkManager`, `wpa_supplicant`).
2. Put your selected interface into monitor mode using proper driver support.
3. Launch the Tauri application with `sudo` while preserving necessary environment variables for Rust (`$PATH`, `CARGO_HOME`, `RUSTUP_HOME`).
4. Restore services when closed.

---

## 🚧 Hardware & Driver Notes

- **Driver Compatibility**: Ensure you are using the correct DKMS drivers for your specific chipset (e.g., `realtek-rtl88xxau-dkms` for RTL8812AU/RTL8814AU). Standard kernel drivers like `rtw88_8814au` can be temperamental with monitor mode packet injection.
- **Validation**: If monitor mode works but no packets are captured, verify hardware viability with `tcpdump -i wlan0` or by testing with standalone tools like Wifite.
- **Missing Interface**: If the launcher fails to find an interface, ensure your USB adapter is physically connected and passed through to your Kali VM.

---

## 📖 Developer Diary: Building Aether (April 2026)

*This project serves as a living diary of our development journey.*

### The Great WSL2 Migration
We initially attempted to run Aether entirely within Windows Subsystem for Linux (WSL2) to provide a seamless Windows-native experience. We built a PowerShell script (`scripts/attach_wsl_wifi.ps1`) to leverage `usbipd-win`. 
However, deep dive diagnostics revealed that DKMS (Dynamic Kernel Module Support) failed to compile `realtek-rtl8814au-dkms` against the active WSL kernel (`6.6.87.2-microsoft-standard-WSL2`) due to missing build trees and undefined kernel symbols. Recompiling custom WSL2 kernels specifically for out-of-tree Wi-Fi injection drivers proved too fragile. 

**The Solution:** We scrapped the entire "WSL nonsense" and migrated the project to a **Native Kali Linux VM** via VMware Workstation. By letting the hypervisor handle the USB pass-through, the native Kali DKMS packages now compile perfectly against a standard kernel, giving us a fully functioning, monitor-mode capable `wlan0` interface. 

### The Monitor Mode & CAP_NET_RAW Struggles
Even after moving to the VM, we hit immediate blockers with `wlan0` packet captures. Specifically, Tauri threw `CAPTURE_ERROR: Failed to activate capture on 'wlan0': libpcap error: Attempt to create packet socket failed - CAP_NET_RAW may be required.`
- **The Culprit:** Rust applications launched via Tauri need elevated privileges to bind raw packet sockets (`libpcap`). We updated the `aether.sh` launcher to wrap `cargo tauri dev` with `sudo -E` to ensure the Rust backend inherits the necessary capabilities while maintaining the user's `$PATH` for Node and Cargo.
- **RF Weirdness & Driver Quirks:** Even in monitor mode, `airmon-ng` and initial scans yielded nothing, despite seeing APs perfectly fine in managed mode. This led to us utilizing tools like Wifite standalone to validate that the hardware actually worked, while dealing with extreme 2.4GHz RF interference in the environment (a bizarre 55dBm hump on channel 11 blowing out adjacent channels).

### UI Overhaul: The `shadcn/ui` Migration
The user rightly pointed out: *"please dont build your own ui components when you could just use know great lookkng libraries right?"*
We entirely scrapped our custom CSS and Aether-specific utility classes, transitioning to a pure **`shadcn/ui`** dark theme.
- **Components Installed:** Over 14 core components (Button, Card, Badge, Table, Tabs, Dialog, Input, Select, Switch, etc.) were deeply integrated.
- **Page Expansion:** We scaffolded 4 entirely new pages (`/recon`, `/attack`, `/sniffer`, `/tools`) to expand our offensive posture, building out the sidebar to 10 distinct navigation items. 
- **The Result:** The app immediately felt more robust, responsive, and maintainable. We flirted with migrating to Material-UI (`MUI`) momentarily but committed to the sleek, modern aesthetic `shadcn/ui` provided.

### The Offensive Paradigm Shift: Marauder & Bettercap Inspiration
During debugging, we used tools like Wifite and Bettercap to manually test our hardware. While Aether does **not** run these tools under the hood, we recognized their power and elegance. 
- **The Goal:** We are modeling Aether's native offensive capabilities after the excellent [ESP32 Marauder](https://github.com/justcallmekoko/ESP32Marauder) (Beacon spam, deauths, karma attacks) and Bettercap workflows. 
- Rather than reinventing the wheel blindly, we aim to build our own high-speed Rust orchestrator that matches the capabilities of these proven tools.

### Next Steps: The Great Consolidation
Our immediate next objective is merging the scattered offensive tabs (`Audit.tsx`, `Attack.tsx`) into a single, cohesive **Offensive Suite**. This unified interface will house 1-click captures, targeted deauths, and advanced injections in one intuitive dashboard.

---

## 📁 Project Structure

```text
Aether_wifi/
├── 📂 src/                          # ⚛️ React Frontend
│   ├── 📂 components/
│   │   ├── 📂 layout/               # AppShell, Sidebar
│   │   └── 📂 ui/                   # shadcn/ui components (Radix primitives + Tailwind)
│   ├── 📂 hooks/
│   │   └── 📄 useBeaconCapture.tsx  # Shared state for captured data
│   ├── 📂 pages/
│   │   ├── 📄 Dashboard.tsx         # Network scan table
│   │   ├── 📄 Spectrum.tsx          # Channel spectrum analyzer
│   │   ├── 📄 Hunt.tsx              # RSSI target tracker
│   │   ├── 📄 Audit.tsx             # [PLANNED MERGE] WPA Handshake capture
│   │   ├── 📄 Attack.tsx            # [PLANNED MERGE] Beacon spam, injection attacks
│   │   ├── 📄 Recon.tsx             # Network discovery & port scanning
│   │   ├── 📄 Sniffer.tsx           # Low-level packet monitor
│   │   ├── 📄 Tools.tsx             # Utility suite
│   │   ├── 📄 EnvironmentDoctor.tsx # System diagnostics
│   │   └── 📄 Settings.tsx          # Interface management
│   ├── 📄 App.tsx                   # React Router
│   └── 📄 index.css                 # Global styles (shadcn/ui dark theme)
├── 📂 src-tauri/                    # 🦀 Rust Backend
│   ├── 📂 src/
│   │   ├── 📂 commands/             # Tauri IPC endpoints
│   │   ├── 📂 network/              # Core Logic (Sniffing, Attacks, Scans)
│   │   ├── 📄 error.rs              # Custom error handling
│   │   └── 📄 main.rs               # Entry point
│   ├── 📄 Cargo.toml
│   └── 📄 tauri.conf.json
├── 📄 aether.sh                     # Linux Privileged Launcher (sudo wrapper)
└── 📄 package.json                  # Entry scripts
```

---

## ⚠️ Security Notice

> [!WARNING]
> This tool contains offensive security capabilities (**deauthentication attacks**, **packet injection**, **evil portals**).
>
> **Use only on networks you own or have explicit written authorization to test.**
>
> Unauthorized use may violate federal and local laws. The authors assume no liability for misuse of this software.

---

## 📜 License

This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.

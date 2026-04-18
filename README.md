<div align="center">

# 🌐 Aether

### Modern WiFi Auditing & Analysis

**A cross-platform, high-performance WiFi analyzer and auditing tool built with Tauri v2, React, and Rust.**

*Bringing the fluid UX of modern Android WiFi analyzers to the desktop, combined with the offensive capabilities of `aircrack-ng` and `Sparrow-WiFi`.*

[![Tauri](https://img.shields.io/badge/Tauri-v2.0-24C8D8?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB?style=for-the-badge&logo=react&logoColor=black)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-Stable-DEA584?style=for-the-badge&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.x-3178C6?style=for-the-badge&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind-v4-38B2AC?style=for-the-badge&logo=tailwind-css&logoColor=white)](https://tailwindcss.com/)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20WSL2-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://www.linux.org/)

---

<img src="assets/aether-demo.gif" alt="Aether Demo" width="100%" style="border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.3);">

*Real-time WiFi spectrum analysis and network auditing in action*

</div>

---

## ✨ Features

<table>
<tr>
<td width="50%">

### 📊 Dashboard

Real-time network table showing **BSSID**, **SSID**, **Channel**, and **RSSI** with color-coded signal strength bars and sortable columns.

</td>
<td width="50%">

### 📡 Spectrum Analyzer

Overlapping parabolic curves across **2.4 GHz**, **5 GHz**, and **6 GHz** bands — visualizes channel congestion like a professional Android WiFi analyzer.

</td>
</tr>
<tr>
<td width="50%">

### 🎯 Hunt Mode

Lock onto a target MAC address and track its **RSSI** over a rolling 60-second timeline. Walk around to triangulate signal origin.

</td>
<td width="50%">

### 🔐 Audit Suite

- **1-Click Handshake Capture**: Deauth → EAPOL listener → save `.pcap` in one button press
- **Manual Deauth**: Send targeted broadcast deauthentication frames
- **EAPOL Capture**: Isolate WPA handshake packets with BPF filter
- **Loot Table**: View all captured `.pcap` files

</td>
</tr>
<tr>
<td width="50%">

### 🩺 Environment Doctor

Auto-detects **WSL2**, missing USB adapters, monitor mode status, and driver compatibility. Provides copy-paste remediation commands.

</td>
<td width="50%">

### ⚙️ Settings

List all network interfaces, toggle **monitor/managed** mode, view driver and PHY info.

</td>
</tr>
</table>

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  ⚛️  React Frontend (Aether-UI)                                  │
│  ├─ 📊 Dashboard      (network table, stats)                     │
│  ├─ 📡 Spectrum       (2.4/5/6 GHz channel viz)                  │
│  ├─ 🎯 Hunt Mode      (targeted RSSI tracker)                    │
│  ├─ 🔐 Audit Suite    (deauth, EAPOL capture)                    │
│  ├─ 🩺 Env Doctor     (WSL2/driver diagnostics)                  │
│  └─ ⚙️  Settings       (interface mode control)                   │
├─────────────────────────────────────────────────────────────────┤
│  🔗 Tauri v2 IPC Bridge                                          │
│  ├─ network_commands   (scan, mode toggle)                       │
│  ├─ capture_commands   (start/stop sniffer)                      │
│  └─ audit_commands     (deauth, EAPOL, 1-click)                  │
├─────────────────────────────────────────────────────────────────┤
│  🦀 Rust Backend (Aether-Core)                                   │
│  ├─ InterfaceScanner   (iw dev / ip link)                        │
│  ├─ ModeController     (managed ↔ monitor)                       │
│  ├─ PacketSniffer      (pcap + 802.11 parse)                     │
│  └─ Audit              (deauth + EAPOL filter)                   │
├─────────────────────────────────────────────────────────────────┤
│  🐧 Linux Kernel (nl80211 / wireless stack)                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🛠️ Tech Stack

| Category | Technology |
|:--------:|:-----------|
| **Framework** | [![Tauri v2](https://img.shields.io/badge/Tauri-v2.0-24C8D8?logo=tauri)](https://v2.tauri.app/) |
| **Frontend** | [![React 19](https://img.shields.io/badge/React-19-61DAFB?logo=react)](https://react.dev/) [![TypeScript](https://img.shields.io/badge/TypeScript-5.x-3178C6?logo=typescript)](https://www.typescriptlang.org/) [![Tailwind CSS](https://img.shields.io/badge/Tailwind-v4-38B2AC?logo=tailwind-css)](https://tailwindcss.com/) |
| **Charting** | [![Apache ECharts](https://img.shields.io/badge/Apache_ECharts-60fps-AA344D?logo=apache-echarts)](https://echarts.apache.org/) |
| **Backend** | [![Rust](https://img.shields.io/badge/Rust-Stable-DEA584?logo=rust)](https://www.rust-lang.org/) |
| **Packet Capture** | [![libpcap](https://img.shields.io/badge/libpcap-pcap-e05d44)](https://www.tcpdump.org/) |
| **UI Components** | [![Radix UI](https://img.shields.io/badge/Radix_UI-Components-7E6DD3)](https://www.radix-ui.com/) [![Lucide](https://img.shields.io/badge/Lucide-Icons-F57059)](https://lucide.dev/) [![Framer Motion](https://img.shields.io/badge/Framer_Motion-Animations-0055FF)](https://www.framer.com/motion/) |

---

## 📸 Screenshots

<div align="center">

| Dashboard | Spectrum Analyzer |
|:---------:|:-----------------:|
| <img src="assets/dashboard.png" alt="Dashboard" width="400"> | <img src="assets/spectrum.png" alt="Spectrum" width="400"> |

| Hunt Mode | Audit Suite |
|:---------:|:-----------:|
| <img src="assets/hunt.png" alt="Hunt Mode" width="400"> | <img src="assets/Audit.png" alt="Audit" width="400"> |

| Environment Doctor | Settings |
|:------------------:|:--------:|
| <img src="assets/health.png" alt="Health" width="400"> | <img src="assets/settings.png" alt="Settings" width="400"> |

</div>

---

## 📋 Prerequisites

| Requirement | Installation |
|:------------|:-------------|
| **Node.js** 18+ | [nodejs.org](https://nodejs.org/) |
| **Rust** (stable) | [rustup.rs](https://rustup.rs) |
| **Tauri v2 CLI** | `cargo install tauri-cli` |
| **libpcap-dev** | `sudo apt install libpcap-dev` |
| **USB WiFi Adapter** | Must support Monitor mode (e.g., Alfa AWUS036ACH, TP-Link Archer T4U) |

### 🐧 WSL2 Users

Install [usbipd-win](https://github.com/doraszama/usbipd-win) on Windows:

```powershell
winget install --exact dorssel.usbipd-win
```

Then use the included auto-config script:

```powershell
.\scripts\attach_wsl_wifi.ps1 -WslHost "kali-linux" -AdapterName "802.11"
```

---

## 🚀 Getting Started

```bash
# 📦 Install frontend dependencies
npm install

# 🧪 Run in development mode (frontend only)
npm run dev

# 🖥️ Run with Tauri (full app with Rust backend)
# Linux WiFi capture uses the launcher to enter monitor mode and start the backend with sudo.
npm run tauri dev

# 🏗️ Build for production
cargo tauri build
```

---

## 📁 Project Structure

```
Aether_wifi/
├── 📂 src/                          # ⚛️  React Frontend
│   ├── 📂 components/
│   │   ├── 📂 layout/               # AppShell, Sidebar
│   │   └── 📂 ui/                   # Tooltip, shared primitives
│   ├── 📂 hooks/
│   │   └── 📄 useBeaconCapture.ts   # Real-time beacon event hook
│   ├── 📂 pages/
│   │   ├── 📄 Dashboard.tsx         # Network scan table
│   │   ├── 📄 Spectrum.tsx          # Channel spectrum analyzer
│   │   ├── 📄 Hunt.tsx              # RSSI target tracker
│   │   ├── 📄 Audit.tsx             # Offensive suite
│   │   ├── 📄 EnvironmentDoctor.tsx # System diagnostics
│   │   └── 📄 Settings.tsx          # Interface management
│   ├── 📂 types/
│   │   └── 📄 capture.ts            # TypeScript interfaces
│   ├── 📄 App.tsx                   # Router
│   ├── 📄 main.tsx                  # Entry point
│   └── 📄 index.css                 # Design system tokens
├── 📂 src-tauri/                    # 🦀 Rust Backend
│   ├── 📂 src/
│   │   ├── 📂 commands/
│   │   │   ├── 📄 network_commands.rs
│   │   │   ├── 📄 capture_commands.rs
│   │   │   └── 📄 audit_commands.rs
│   │   ├── 📂 network/
│   │   │   ├── 📄 interface_scanner.rs
│   │   │   ├── 📄 mode_controller.rs
│   │   │   ├── 📄 packet_sniffer.rs
│   │   │   ├── 📄 audit.rs
│   │   │   └── 📄 types.rs
│   │   ├── 📄 error.rs
│   │   └── 📄 lib.rs
│   ├── 📄 Cargo.toml
│   └── 📄 tauri.conf.json
├── 📂 scripts/
│   └── 📄 attach_wsl_wifi.ps1       # WSL2 USB passthrough script
├── 📂 assets/                       # 🖼️ Screenshots & demos
└── 📄 package.json
```

---

## ⚠️ Security Notice

> [!WARNING]
> This tool contains offensive security capabilities (**deauthentication attacks**, **packet injection**).
>
> **Use only on networks you own or have explicit written authorization to test.**
>
> Unauthorized use may violate federal and local laws. The authors assume no liability for misuse of this software.

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. 🍴 Fork the repository
2. 🔀 Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. 💾 Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. 📤 Push to the branch (`git push origin feature/AmazingFeature`)
5. 📝 Open a Pull Request

---

## 📜 License

This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.

---

<div align="center">

### 🌟 Star this repo if you find it useful

[![Star History Chart](https://api.star-history.com/svg?repos=WontML/Aether_wifi&type=Date)](https://star-history.com/#WontML/Aether_wifi&Date)

---

**Made with ❤️ by [WontML](https://github.com/WontML)**

[![GitHub](https://img.shields.io/badge/GitHub-WontML-181717?style=for-the-badge&logo=github)](https://github.com/WontML)

</div>

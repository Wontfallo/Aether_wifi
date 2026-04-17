PROJECT BLUEPRINT: "AETHER" (Modern WiFi Auditing & Analysis)

1. The Vision

Aether is a cross-platform, high-performance WiFi analyzer and auditing tool. It brings the fluid, intuitive UX of modern Android WiFi analyzers to the desktop, combined with the offensive capabilities of aircrack-ng and Sparrow-WiFi. It handles interface management (Monitor/Managed mode) silently in the background.

1. The Tech Stack (The "No Bullshit" Architecture)

We are abandoning Python/PyQt5 entirely to avoid dependency hell.

- Framework: Tauri v2 (Produces incredibly lightweight desktop apps).

- Frontend (The UI): React 18 + TypeScript + TailwindCSS. For fluid, 60FPS graphing, we will use Apache ECharts or Chart.js (canvas-based, no UI lag when plotting hundreds of networks).

- Backend (The Engine): Rust. Rust will interact directly with the OS network stack. It compiles to a single, standalone binary. No Python environments, no pip installs.

- Packet Capture: The Rust pcap and pnet crates (bindings for libpcap).

1. System Architecture: Split Privilege Model

The biggest issue with Linux WiFi tools is they require sudo, which breaks modern GUIs (Wayland/WSLg). Aether solves this using a Split Architecture:

1. Aether-UI (Unprivileged): The sleek frontend runs as a normal user.

2. Aether-Core (Privileged): A headless Rust daemon that runs as root.

3. The Bridge: The UI communicates with the Core via local IPC (Inter-Process Communication) or WebSockets. The UI says "Scan channel 6", the Core does the dirty work and streams the JSON data back.

4. Feature Modules & UI Layout

Module A: The Dashboard (Active Scan)

- Visual: A beautiful, dark-mode data table (BSSID, SSID, Security, Signal, Channel, Frequency, Vendor).

- Mechanic: Uses standard OS APIs (Windows Native WiFi API, Linux nl80211) to do a "Managed" mode scan. Does not require special drivers. Works out of the box on every PC.

Module B: The Spectrum Analyzer (2.4GHz / 5GHz / 6GHz)

- Visual: Overlapping parabolic curves showing network signal strength across channels (exactly like the Android app).

- Mechanic: Plotted in real-time. Visually exposes channel crowding so users can optimize their home routers.

Module C: "Hunt" Mode (Radar / RSSI Tracker)

- Visual: A real-time scrolling line graph tracking the RSSI (dBm) of a specific, targeted MAC address.

- Mechanic: Requires Monitor mode. The user selects a target. Aether-Core locks the WiFi adapter to the target's channel, sniffs raw 802.11 frames, and plots the signal strength. As you walk around, the line goes up or down.

Module D: The Offensive Suite (Audit Mode)

- Visual: A dedicated panel for Handshake Capture and Deauth attacks.

- Mechanic: Rust parses raw 802.11 frames looking for EAPOL packets (the WPA handshake). It features a "One-Click Capture":

 1. Click a target network.

 2. Aether silently sends a broadcast deauth packet to disconnect clients.

 3. Aether listens for the handshake upon reconnection.

 4. Saves to a clean .pcap file automatically.

 1. The WSL2 "Magic" Pipeline (Auto-Config)

Since we know the WSL2 pain, Aether will have a built-in "Environment Doctor":

- On boot, Aether checks if it's running in WSL.

- If it detects a missing USB adapter, the UI provides a copy-paste PowerShell command (usbipd attach) to the user.

- If it detects missing Monitor mode capabilities, it auto-detects the chipset (e.g., RTL8814AU) and provides the exact terminal command to install the DKMS driver.

---

1. Implementation Roadmap for your AI Agent

Feed this step-by-step to your Antigravity IDE agent to start coding.

PHASE 1: Scaffolding the App

1. Command: "Initialize a new Tauri project using React, TypeScript, and Vite. Set up TailwindCSS for styling."

2. Command: "Create the base UI layout: A left-side navigation sidebar (Dashboard, Spectrum, Hunt, Audit) and a main content area. Use dark mode by default."

PHASE 2: The Rust Backend (Interface Manager)

1. Command: "In the Tauri Rust backend, write a module using the nl80211 netlink protocol (or standard Linux commands via std::process::Command as a fallback) to list all available network interfaces."

2. Command: "Create a Rust function to toggle a specific interface between 'Managed' and 'Monitor' mode using ip link and iw commands, handling error states gracefully."

PHASE 3: The Packet Sniffer

1. Command: "Add the pcap crate to the Rust backend. Write a packet sniffing loop that listens on a specified interface in Monitor mode, parses 802.11 beacon frames, and extracts the BSSID, SSID, Channel, and RSSI."

2. Command: "Create a Tauri command to stream this parsed JSON data to the React frontend in real-time using Tauri events."

PHASE 4: Frontend Data Visualization

1. Command: "Install echarts-for-react. Create a 'Spectrum' component that subscribes to the Tauri packet stream. Plot the WiFi networks on an X-axis of Channels (1-14) and a Y-axis of Signal Strength (-100 to -20 dBm)."

2. Command: "Create the 'Hunt' component. It should take a target MAC address, filter the incoming Tauri packet stream for only that MAC, and plot its RSSI over a rolling 60-second line chart."

PHASE 5: Offensive Capabilities (Advanced)

1. Command: "In the Rust backend, construct a raw 802.11 Deauthentication frame. Create a function that injects this frame using pcap to a specific BSSID."

2. Command: "Write a BPF (Berkeley Packet Filter) in the Rust pcap listener that isolates ether proto 0x888e (EAPOL packets) to detect and save WPA handshakes to disk."

---

 "Execute Phase 1." Let's see what it builds.

1. The Core Engine (Rust & Systems)


Since Aether's backend bypasses Python and directly interacts with the Linux network stack and raw packet parsing, the agent needs strong systems programming context.


- rust-pro: Essential for writing the high-performance Tauri backend and IPC bridge.

- systems-programming-rust-project: Crucial for interacting with standard Linux APIs (like nl80211 or executing child shell processes) securely.

- wireshark-analysis: This is a secret weapon. To write the pcap packet sniffer, the agent needs to know exactly how 802.11 Beacon frames and EAPOL (WPA handshake) packets are structured. This skill gives it that packet-level context.

- network-engineer: Provides the fundamental knowledge of WiFi channels, frequencies (2.4/5/6GHz), dBm signal strength, and MAC address routing.

2. The Frontend (React & UI/UX)


We want this to look like a sleek, modern, 60FPS application, not a clunky 2012 Python script.


- react-best-practices: For building out the dashboard, state management, and subscribing to the real-time Tauri events stream without causing memory leaks.

- typescript-expert: To ensure the data types coming from the Rust backend perfectly map to the React frontend (e.g., standardizing the JSON payload for WiFi networks).

- tailwind-patterns: Essential for quickly mocking up the dark-mode, hacker-aesthetic dashboard.

- ui-ux-pro-max: Because we specifically want the smooth, intuitive UX of modern Android WiFi analyzers, this will help the agent architect the layout and overlapping parabolic charts cleanly.

3. The Auto-Config "Magic" (Scripts & OS)


To achieve the "No Bullshit" user experience where it handles WSL2 and interface states automatically, the agent needs to write perfect cross-platform glue code.


- powershell-windows: Used specifically to write the usbipd attach auto-config script so Windows users don't have to touch a terminal to pass their adapter to the app.

- linux-shell-scripting: Required for writing the fail-safes that bring the network adapter down, switch it to monitor or managed mode via iw, and bring it back up.

4. The Offensive Suite (Security)


For the "Hunt" and "Audit" modules (Deauth attacks and Handshake capturing).


- ethical-hacking-methodology: Gives the agent the context of why we are capturing these packets and the standard procedures for isolating a target without bringing down a whole network.

- software-architecture: To successfully implement the "Split Privilege Model" (where the UI runs as a normal user but the Rust packet sniffer runs as root).

How to proceed in the IDE:



2. Paste the Project Blueprint from my previous message.

3. Give it the prompt: "We are building Aether based on this blueprint. Initialize Phase 1: The Tauri + React + Vite + Tailwind scaffolding."

Let it rip. It should immediately generate the foundational workspace!
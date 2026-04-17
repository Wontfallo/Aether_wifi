PHASE 1: App Scaffolding & The "Anti-Slop" UI


Goal: Build the pristine, dark-mode desktop shell without writing any backend logic yet.

Tell the Agent:


"Agent, for this phase, heavily utilize your frontend-ui-dark-ts, tailwind-design-system, radix-ui-design-system, react-ui-patterns, and frontend-design skills.


Initialize a Tauri v2 project with React, TypeScript, and Vite. Set up TailwindCSS using a strict dark-mode design system. Do not use inline styles or generic layouts. I want a premium, high-density dashboard aesthetic.


Build the base layout shell: A slim left-side navigation rail containing icons for 'Dashboard', 'Spectrum', 'Hunt', and 'Audit', and a main content area. Scaffold these empty React components cleanly."



---

PHASE 2: The Rust Backend (Interface Manager)


Goal: Write the privileged Linux system calls to manage the WiFi adapter modes.

Tell the Agent:


"Agent, shift your focus. For this phase, rely entirely on your rust-pro, systems-programming-rust-project, linux-shell-scripting, and software-architecture skills.


We are building the 'Aether-Core' backend in Tauri's Rust environment. I need a modular Rust setup that interacts with the Linux OS.



1. Write a Rust module to list available network interfaces (fallback to parsing standard ip link or iwconfig commands if nl80211 bindings are too heavy).

2. Create a Rust function that takes an interface name (e.g., 'wlan0') and toggles it cleanly between 'Managed' and 'Monitor' mode. Handle OS-level permissions and errors gracefully. Keep the architecture clean."



---

PHASE 3: The Packet Sniffer & Data Bridge


Goal: Sniff raw WiFi frames and stream them to the UI in real-time.

Tell the Agent:


"Agent, activate your rust-pro, wireshark-analysis, network-engineer, and typescript-expert skills.


We are building the packet sniffing engine.



1. Add the Rust pcap crate to the backend.

2. Write a listener that opens a monitor-mode interface and specifically parses 802.11 Beacon frames. Extract the BSSID, SSID, Channel, and RSSI (Signal Strength). Use your Wireshark skills to ensure byte-offsets for the radiotap header and 802.11 frames are correct.

3. Create a Tauri IPC event stream that emits this parsed data as a structured JSON payload to the frontend.

4. Write the strict TypeScript interfaces for this payload in the React frontend."



---

PHASE 4: Frontend Data Visualization (The Radar)


Goal: Plot the live WiFi data into fluid charts.

Tell the Agent:


"Agent, switch back to frontend mode. Utilize react-best-practices, react-ui-patterns, and typescript-expert.


We need to visualize the real-time Tauri event stream we just built without causing React re-render lag.



1. In the 'Dashboard' component, build a sleek, sortable data table that updates with the live BSSID/SSID/RSSI data.

2. In the 'Spectrum' component, use a canvas-based charting library (like ECharts or Chart.js) to draw overlapping parabolic curves for the WiFi networks based on their Channel (X-axis) and RSSI (Y-axis).

3. Ensure the React hooks managing this data stream are perfectly memoized to prevent memory leaks."



---

PHASE 5: The Offensive Suite (Deauth & Handshakes)


Goal: Implement the raw packet injection and WPA handshake capture.

Tell the Agent:


"Agent, this is critical. Activate your ethical-hacking-methodology, wireshark-analysis, and systems-programming-rust-project skills.


We are building the 'Audit' module in Rust.



1. Write a Rust function using pcap that constructs and injects a raw 802.11 Broadcast Deauthentication frame targeting a specific BSSID.

2. Write a separate pcap filter logic that isolates ether proto 0x888e (EAPOL packets) to detect the 4-way WPA handshake.

3. Write a function to save these captured EAPOL packets to a .pcap file on the disk."



---

PHASE 6: The WSL2 Auto-Config Script


Goal: Create the seamless Windows-to-Linux USB passthrough.

Tell the Agent:


"Agent, use your powershell-windows and linux-shell-scripting skills.


Write a robust PowerShell script intended to run on the Windows host. This script must seamlessly wake the WSL2 Kali instance, bind a designated USB WiFi adapter using usbipd, attach it to WSL, and handle common errors (like the adapter not being plugged in or the WSL instance being asleep). Make the script clean, silent, and heavily commented."



---

Pro-Tip for working with the Agent:


If the agent starts outputting sloppy UI code in Phase 4, immediately stop it and say: "You are forgetting your frontend-ui-dark-ts and radix-ui-design-system skills. Re-write that component using strict design system primitives."

Drop that Phase 1 prompt into the IDE right now and watch it lay the foundation!
#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════
#  AETHER WiFi Auditor — One-Command Launcher
# ═══════════════════════════════════════════════════════
#
#  Usage (from Windows):
#    wsl -d kali-linux -- bash /mnt/c/Users/WontML/dev/Aether_wifi/aether.sh
#
#  Or from inside Kali:
#    bash ~/Aether_wifi/aether.sh
#
# ═══════════════════════════════════════════════════════

set -e

IFACE="${AETHER_IFACE:-wlan0}"
PROJECT_DIR="/mnt/c/Users/WontML/dev/Aether_wifi"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${CYAN}"
echo "  ╔══════════════════════════════════════╗"
echo "  ║        ◈ AETHER WiFi Auditor ◈       ║"
echo "  ║          Launching System...          ║"
echo "  ╚══════════════════════════════════════╝"
echo -e "${NC}"

# ── 1. Source Rust toolchain ──
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

if ! command -v cargo &>/dev/null; then
    echo -e "${RED}[ERROR] cargo not found. Install Rust first:${NC}"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# ── 2. Check WiFi adapter ──
echo -e "${YELLOW}[1/4]${NC} Checking WiFi adapter (${IFACE})..."
if ! ip link show "$IFACE" &>/dev/null; then
    echo -e "${RED}[ERROR] Interface '$IFACE' not found.${NC}"
    echo "  Available interfaces:"
    ip link show | grep -E "^[0-9]+" | awk '{print "    " $2}'
    echo ""
    echo "  Set a different interface: AETHER_IFACE=wlanX bash aether.sh"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} Found $IFACE"

# ── 3. Set monitor mode if needed ──
echo -e "${YELLOW}[2/4]${NC} Ensuring monitor mode..."
CURRENT_MODE=$(iw dev "$IFACE" info 2>/dev/null | grep type | awk '{print $2}')
if [ "$CURRENT_MODE" = "monitor" ]; then
    echo -e "  ${GREEN}✓${NC} Already in monitor mode"
else
    echo -e "  Setting $IFACE to monitor mode (requires sudo)..."
    sudo ip link set "$IFACE" down
    sudo iw "$IFACE" set type monitor
    sudo ip link set "$IFACE" up
    echo -e "  ${GREEN}✓${NC} Monitor mode activated"
fi

# ── 4. Set channel 6 (busy 2.4GHz channel for best beacon coverage) ──
echo -e "${YELLOW}[3/4]${NC} Setting channel 6 (2.4 GHz)..."
sudo iw dev "$IFACE" set channel 6 2>/dev/null || true
echo -e "  ${GREEN}✓${NC} Channel 6 (2437 MHz)"

# ── 5. Allow X11 access for root & launch ──
echo -e "${YELLOW}[4/4]${NC} Launching Aether..."
xhost +local:root &>/dev/null 2>&1 || true

cd "$PROJECT_DIR"

echo -e ""
echo -e "  ${GREEN}══════════════════════════════════════${NC}"
echo -e "  ${GREEN}  Aether is starting...               ${NC}"
echo -e "  ${GREEN}  Interface: $IFACE (Monitor Mode)     ${NC}"
echo -e "  ${GREEN}  Close the window to stop.            ${NC}"
echo -e "  ${GREEN}══════════════════════════════════════${NC}"
echo ""

# Launch Tauri dev (with sudo for pcap access)
WEBKIT_DISABLE_COMPOSITING_MODE=1 \
    sudo -E "$(which cargo)" tauri dev 2>&1

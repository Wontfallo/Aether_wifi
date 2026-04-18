#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════
#  AETHER WiFi Auditor — One-Command Launcher
# ═══════════════════════════════════════════════════════
#
#  Usage:
#    bash ./aether.sh
#
#  Optional overrides:
#    AETHER_IFACE=wlan1 bash ./aether.sh
#    AETHER_RESTORE_MANAGED=0 bash ./aether.sh
#
# ═══════════════════════════════════════════════════════

set -euo pipefail

IFACE="${AETHER_IFACE:-wlan0}"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="${AETHER_PROJECT_DIR:-$SCRIPT_DIR}"
RESTORE_MANAGED="${AETHER_RESTORE_MANAGED:-1}"
NEEDS_CLEANUP=0
NETWORKMANAGER_WAS_ACTIVE=0
WPASUPPLICANT_WAS_ACTIVE=0
INITIAL_CHANNEL=""

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

service_is_active() {
    local service="$1"
    command -v systemctl &>/dev/null && systemctl is-active --quiet "$service"
}

restore_interface_state() {
    if [ "$RESTORE_MANAGED" != "1" ] || [ "$NEEDS_CLEANUP" -ne 1 ]; then
        return
    fi

    echo -e ""
    echo -e "${YELLOW}[cleanup]${NC} Restoring ${IFACE} to managed mode..."

    if command -v airmon-ng &>/dev/null; then
        sudo airmon-ng stop "$IFACE" &>/dev/null || true
    else
        sudo ip link set "$IFACE" down &>/dev/null || true
        sudo iw "$IFACE" set type managed &>/dev/null || true
        sudo ip link set "$IFACE" up &>/dev/null || true
    fi

    if command -v nmcli &>/dev/null; then
        # IFACE might be wlan0mon, restore base name
        local BASE_IFACE="${IFACE%mon}"
        sudo nmcli device set "$BASE_IFACE" managed yes &>/dev/null || true
    fi

    if command -v systemctl &>/dev/null; then
        if [ "$NETWORKMANAGER_WAS_ACTIVE" -eq 1 ]; then
            sudo systemctl restart NetworkManager &>/dev/null || true
        fi

        if [ "$WPASUPPLICANT_WAS_ACTIVE" -eq 1 ]; then
            sudo systemctl restart wpa_supplicant &>/dev/null || true
        fi
    fi

    echo -e "  ${GREEN}✓${NC} Restored to managed mode"
}

trap restore_interface_state EXIT INT TERM

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

if service_is_active NetworkManager; then
    NETWORKMANAGER_WAS_ACTIVE=1
fi

if service_is_active wpa_supplicant; then
    WPASUPPLICANT_WAS_ACTIVE=1
fi

INITIAL_CHANNEL="$(iw dev "$IFACE" info 2>/dev/null | awk '/channel / { print $2; exit }')"

# ── 2. Check WiFi adapter ──
echo -e "${YELLOW}[1/4]${NC} Checking WiFi adapter (${IFACE})..."
if ! ip link show "$IFACE" &>/dev/null; then
    if command -v airmon-ng &>/dev/null && ip link show "${IFACE}mon" &>/dev/null; then
        echo -e "  Found leftover monitor interface ${IFACE}mon, attempting recovery..."
        sudo airmon-ng stop "${IFACE}mon" &>/dev/null || true
    fi

    if ! ip link show "$IFACE" &>/dev/null; then
        echo -e "${RED}[ERROR] Interface '$IFACE' not found.${NC}"
        echo "  Available interfaces:"
        ip link show | grep -E "^[0-9]+" | awk '{print "    " $2}'
        echo ""
        echo "  Set a different interface: AETHER_IFACE=wlanX bash ./aether.sh"
        exit 1
    fi
fi
echo -e "  ${GREEN}✓${NC} Found $IFACE"
NEEDS_CLEANUP=1

# ── 3. Kill interfering services & activate monitor mode ──
echo -e "${YELLOW}[2/4]${NC} Ensuring clean monitor mode..."

# Set regulatory domain for proper txpower
sudo iw reg set US &>/dev/null || true

echo -e "  Stopping interfering services..."
if command -v airmon-ng &>/dev/null; then
    sudo airmon-ng check kill &>/dev/null || true
else
    sudo systemctl stop NetworkManager &>/dev/null || true
    sudo systemctl stop wpa_supplicant &>/dev/null || true
fi
if command -v nmcli &>/dev/null; then
    sudo nmcli device set "$IFACE" managed no &>/dev/null || true
fi

# Use airmon-ng for monitor mode (handles driver quirks better than iw)
if command -v airmon-ng &>/dev/null; then
    sudo airmon-ng stop "$IFACE" &>/dev/null || true
    sleep 0.5
    sudo airmon-ng start "$IFACE" &>/dev/null || true
    # Detect if interface was renamed (e.g. wlan0 -> wlan0mon)
    if ip link show "${IFACE}mon" &>/dev/null 2>&1; then
        IFACE="${IFACE}mon"
        echo -e "  Interface renamed to ${IFACE}"
    fi
else
    sudo ip link set "$IFACE" down
    sudo iw "$IFACE" set type monitor
    sudo ip link set "$IFACE" up
fi
echo -e "  ${GREEN}✓${NC} Monitor mode activated on ${IFACE} (services killed)"

# ── 4. Set a sensible initial channel ──
TARGET_CHANNEL="${INITIAL_CHANNEL:-6}"
echo -e "${YELLOW}[3/4]${NC} Setting initial channel ${TARGET_CHANNEL}..."
sudo iw dev "$IFACE" set channel "$TARGET_CHANNEL" 2>/dev/null || true
echo -e "  ${GREEN}✓${NC} Channel ${TARGET_CHANNEL}"

# ── 5. Allow X11 access for root & launch ──
echo -e "${YELLOW}[4/4]${NC} Launching Aether..."
if command -v xhost &>/dev/null && [ -n "${DISPLAY:-}" ]; then
    xhost +local:root &>/dev/null 2>&1 || true
fi

cd "$PROJECT_DIR"

echo -e ""
echo -e "  ${GREEN}══════════════════════════════════════${NC}"
echo -e "  ${GREEN}  Aether is starting...               ${NC}"
echo -e "  ${GREEN}  Interface: $IFACE (Monitor Mode)     ${NC}"
echo -e "  ${GREEN}  Close the window to stop.            ${NC}"
echo -e "  ${GREEN}══════════════════════════════════════${NC}"
echo ""

TAURI_BIN="$PROJECT_DIR/node_modules/.bin/tauri"
CARGO_BIN_DIR="$HOME/.cargo/bin"
if [ ! -x "$TAURI_BIN" ]; then
    echo -e "${RED}[ERROR] Tauri CLI not found at ${TAURI_BIN}.${NC}"
    echo "  Run 'npm install' in $PROJECT_DIR first."
    exit 1
fi

# Launch Tauri dev (with sudo for pcap access)
WEBKIT_DISABLE_COMPOSITING_MODE=1 \
    sudo -E env \
        "PATH=$CARGO_BIN_DIR:$PATH" \
        "CARGO_HOME=$HOME/.cargo" \
        "RUSTUP_HOME=$HOME/.rustup" \
        "AETHER_MONITOR_IFACE=$IFACE" \
        "$TAURI_BIN" dev 2>&1

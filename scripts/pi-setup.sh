#!/usr/bin/env bash
# pi-setup.sh — Install and configure zeroclaw on a Raspberry Pi
#
# Usage (on the Pi):
#   curl -sSL <raw-github-url>/scripts/pi-setup.sh | bash
#   # or:
#   ./scripts/pi-setup.sh [/path/to/zeroclaw-binary]
#
# What this does:
#   1. Copies the zeroclaw binary to /usr/local/bin (or builds from source)
#   2. Runs the onboard wizard
#   3. Installs a systemd user service for always-on operation

set -euo pipefail

INSTALL_DIR="/usr/local/bin"
BINARY_NAME="zeroclaw"
REPO_URL="https://github.com/Ibouseye04/zeroclaw"

info()  { printf '\033[1;34m[info]\033[0m  %s\n' "$*"; }
ok()    { printf '\033[1;32m[ok]\033[0m    %s\n' "$*"; }
warn()  { printf '\033[1;33m[warn]\033[0m  %s\n' "$*"; }
err()   { printf '\033[1;31m[error]\033[0m %s\n' "$*" >&2; exit 1; }

# ── Check architecture ──────────────────────────────────────────
ARCH=$(uname -m)
if [[ "$ARCH" != "aarch64" && "$ARCH" != "armv7l" ]]; then
    err "This script is for ARM devices (got: $ARCH)"
fi

if [[ "$ARCH" == "armv7l" ]]; then
    warn "32-bit ARM detected. For best performance, use 64-bit Raspberry Pi OS."
    warn "See: https://www.raspberrypi.com/software/"
fi

# ── Option 1: Binary provided as argument ───────────────────────
if [[ -n "${1:-}" && -f "$1" ]]; then
    info "Installing provided binary: $1"
    sudo install -m 755 "$1" "$INSTALL_DIR/$BINARY_NAME"
    ok "Installed $INSTALL_DIR/$BINARY_NAME"

# ── Option 2: Binary already on PATH ───────────────────────────
elif command -v "$BINARY_NAME" &>/dev/null; then
    info "zeroclaw already on PATH: $(command -v $BINARY_NAME)"

# ── Option 3: Build from source ────────────────────────────────
else
    info "No binary found — building from source"

    # Install Rust if needed
    if ! command -v cargo &>/dev/null; then
        info "Installing Rust toolchain..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
    fi

    # Install build deps
    if command -v apt-get &>/dev/null; then
        info "Installing build dependencies..."
        sudo apt-get update -qq
        sudo apt-get install -y -qq build-essential pkg-config
    fi

    # Clone and build
    BUILD_DIR=$(mktemp -d)
    info "Cloning zeroclaw into $BUILD_DIR..."
    git clone --depth 1 "$REPO_URL" "$BUILD_DIR/zeroclaw"
    cd "$BUILD_DIR/zeroclaw"

    info "Building release binary (this will take a while on Pi hardware)..."
    cargo build --release

    sudo install -m 755 "target/release/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
    ok "Installed $INSTALL_DIR/$BINARY_NAME"

    # Cleanup
    rm -rf "$BUILD_DIR"
    cd "$HOME"
fi

# ── Verify ──────────────────────────────────────────────────────
if ! command -v "$BINARY_NAME" &>/dev/null; then
    err "$BINARY_NAME not found on PATH after install"
fi

VERSION=$("$BINARY_NAME" --version 2>/dev/null || echo "unknown")
ok "zeroclaw version: $VERSION"

# ── Onboard ─────────────────────────────────────────────────────
if [[ ! -f "$HOME/.zeroclaw/config.toml" ]]; then
    info "Running onboard wizard..."
    "$BINARY_NAME" onboard
else
    info "Config already exists at ~/.zeroclaw/config.toml — skipping onboard"
fi

# ── Install systemd service ─────────────────────────────────────
info "Installing systemd user service..."
"$BINARY_NAME" service install 2>/dev/null || true

# Enable lingering so the user service survives logout
if command -v loginctl &>/dev/null; then
    info "Enabling user lingering (service runs without active login)..."
    sudo loginctl enable-linger "$(whoami)" 2>/dev/null || true
fi

ok "Setup complete!"
echo ""
echo "  Quick start:"
echo "    zeroclaw agent                    # Interactive chat"
echo "    zeroclaw agent -m 'hello'         # Single message"
echo "    zeroclaw service start            # Start background daemon"
echo "    zeroclaw service status            # Check daemon status"
echo "    zeroclaw doctor                   # Run diagnostics"
echo ""
echo "  Config: ~/.zeroclaw/config.toml"
echo "  Logs:   ~/.zeroclaw/logs/"
echo ""

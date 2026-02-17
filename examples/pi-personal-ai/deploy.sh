#!/usr/bin/env bash
# deploy.sh — Deploy ZeroClaw as a personal always-on AI on Raspberry Pi
#
# Prerequisites:
#   - zeroclaw binary installed at /usr/local/bin/zeroclaw
#   - Run: zeroclaw onboard (or copy config.toml to ~/.zeroclaw/config.toml)
#
# Usage:
#   ./deploy.sh                     # Full deploy
#   ./deploy.sh --workspace-only    # Just copy workspace files
#   ./deploy.sh --telegram           # Set up Telegram bot interactively

set -euo pipefail

ZEROCLAW_DIR="$HOME/.zeroclaw"
WORKSPACE_DIR="$ZEROCLAW_DIR/workspace"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

info()  { printf '\033[1;34m[info]\033[0m  %s\n' "$*"; }
ok()    { printf '\033[1;32m[ok]\033[0m    %s\n' "$*"; }
warn()  { printf '\033[1;33m[warn]\033[0m  %s\n' "$*"; }
err()   { printf '\033[1;31m[error]\033[0m %s\n' "$*" >&2; exit 1; }

# ── Verify zeroclaw is installed ────────────────────────────────
command -v zeroclaw &>/dev/null || err "zeroclaw not found. Install it first (see scripts/pi-setup.sh)"

# ── Create workspace ────────────────────────────────────────────
setup_workspace() {
    info "Setting up workspace at $WORKSPACE_DIR"
    mkdir -p "$WORKSPACE_DIR/memory"
    mkdir -p "$WORKSPACE_DIR/skills"
    mkdir -p "$WORKSPACE_DIR/logs"

    # Copy seed personality files (don't overwrite existing)
    for file in SOUL.md IDENTITY.md USER.md AGENTS.md TOOLS.md HEARTBEAT.md MEMORY.md BOOTSTRAP.md; do
        if [[ ! -f "$WORKSPACE_DIR/$file" ]]; then
            if [[ -f "$SCRIPT_DIR/workspace/$file" ]]; then
                cp "$SCRIPT_DIR/workspace/$file" "$WORKSPACE_DIR/$file"
                ok "Created $file"
            fi
        else
            info "Skipping $file (already exists)"
        fi
    done
}

# ── Copy config template ───────────────────────────────────────
setup_config() {
    if [[ ! -f "$ZEROCLAW_DIR/config.toml" ]]; then
        if [[ -f "$SCRIPT_DIR/config.toml" ]]; then
            cp "$SCRIPT_DIR/config.toml" "$ZEROCLAW_DIR/config.toml"
            ok "Created config.toml template"
            warn "Edit ~/.zeroclaw/config.toml to add your API key and channel tokens!"
        fi
    else
        info "Skipping config.toml (already exists)"
    fi
}

# ── Install systemd service ─────────────────────────────────────
setup_service() {
    info "Installing systemd service..."
    zeroclaw service install 2>/dev/null || warn "Service install returned non-zero (may already exist)"

    # Enable lingering so service runs without active login
    if command -v loginctl &>/dev/null; then
        info "Enabling user lingering..."
        sudo loginctl enable-linger "$(whoami)" 2>/dev/null || true
    fi

    ok "Systemd service configured"
    echo "    Start:  zeroclaw service start"
    echo "    Status: zeroclaw service status"
    echo "    Stop:   zeroclaw service stop"
}

# ── Telegram setup helper ───────────────────────────────────────
setup_telegram() {
    echo ""
    echo "=== Telegram Bot Setup ==="
    echo ""
    echo "1. Open Telegram and message @BotFather"
    echo "2. Send: /newbot"
    echo "3. Pick a name (e.g. 'ZeroClaw') and username (e.g. 'zeroclaw_pi_bot')"
    echo "4. Copy the bot token"
    echo ""

    read -rp "Paste your Telegram bot token: " bot_token
    if [[ -z "$bot_token" ]]; then
        warn "No token provided, skipping Telegram setup"
        return
    fi

    read -rp "Your Telegram username (without @): " username
    if [[ -z "$username" ]]; then
        warn "No username provided, skipping Telegram setup"
        return
    fi

    # Append Telegram config
    if grep -q "channels_config.telegram" "$ZEROCLAW_DIR/config.toml" 2>/dev/null; then
        warn "Telegram config already exists in config.toml — edit manually if needed"
    else
        # Ensure [channels_config] section exists (cli = true required by parser)
        if ! grep -q '^\[channels_config\]' "$ZEROCLAW_DIR/config.toml" 2>/dev/null; then
            cat >> "$ZEROCLAW_DIR/config.toml" << TOML

[channels_config]
cli = true
TOML
        fi
        cat >> "$ZEROCLAW_DIR/config.toml" << TOML

[channels_config.telegram]
bot_token = "$bot_token"
allowed_users = ["$username"]
TOML
        ok "Telegram bot configured! Message your bot to start chatting."
    fi
}

# ── Main ────────────────────────────────────────────────────────
main() {
    echo ""
    echo "  🦀 ZeroClaw Personal AI — Pi Deployment"
    echo ""

    case "${1:-}" in
        --workspace-only)
            setup_workspace
            ;;
        --telegram)
            setup_telegram
            ;;
        *)
            mkdir -p "$ZEROCLAW_DIR"
            setup_config
            setup_workspace
            setup_service

            echo ""
            echo "=== Next Steps ==="
            echo ""
            echo "1. Edit config:     nano ~/.zeroclaw/config.toml"
            echo "   - Add your OpenRouter API key (get one at openrouter.ai/keys)"
            echo "   - Uncomment and configure Telegram (or run: $0 --telegram)"
            echo ""
            echo "2. Personalize:     nano ~/.zeroclaw/workspace/USER.md"
            echo "   - Fill in your name, timezone, preferences"
            echo ""
            echo "3. Test:            zeroclaw agent -m 'Hello, ZeroClaw!'"
            echo ""
            echo "4. Go live:         zeroclaw service start"
            echo ""
            echo "5. Text your bot on Telegram and start building your AI."
            echo ""
            ;;
    esac
}

main "$@"

# HEARTBEAT.md — Periodic Tasks

# These tasks run every 30 minutes (configurable in config.toml).
# Each line starting with "- " spawns an agent session.
# Results are saved to daily memory automatically.
#
# Uncomment tasks below as you set up each integration.

# --- Email (requires Composio Gmail integration) ---
# - Check my email for anything urgent or important. Summarize new messages briefly.

# --- Weather ---
# - What's the weather forecast for today? Keep it to one sentence.

# --- System Health ---
- Run `df -h / && free -h && uptime` and note if disk is above 80% or memory is low.

# --- Daily Summary (enable after a few days of use) ---
# - Review today's memory entries and write a 3-line summary to MEMORY.md under "Open Loops"

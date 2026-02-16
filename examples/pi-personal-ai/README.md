# Pi Personal AI

Turn your Raspberry Pi 4 into an always-on personal AI assistant with its own identity, memory, and messaging channels.

## What You Get

- **Persistent personality** that evolves over time (SOUL.md, IDENTITY.md)
- **Long-term memory** via SQLite with hybrid search
- **Text it anytime** via Telegram (or Discord/WhatsApp)
- **Proactive tasks** via heartbeat (check email, system health, etc.)
- **Always-on daemon** with systemd auto-restart
- **3.4MB binary**, <10ms startup, <10MB RAM

## Quick Deploy

```bash
# 1. Install zeroclaw on the Pi (see main README)
# 2. Copy this example to the Pi
scp -r examples/pi-personal-ai pi@<pi-ip>:~/

# 3. On the Pi:
cd ~/pi-personal-ai
./deploy.sh

# 4. Add your API key
nano ~/.zeroclaw/config.toml

# 5. Set up Telegram
./deploy.sh --telegram

# 6. Fill in your info
nano ~/.zeroclaw/workspace/USER.md

# 7. Test it
zeroclaw agent -m "Hello, ZeroClaw!"

# 8. Go live
zeroclaw service start
```

## Files

| File | Purpose |
|------|---------|
| `config.toml` | Pi-optimized config template |
| `deploy.sh` | Automated deployment script |
| `workspace/SOUL.md` | Core personality definition |
| `workspace/IDENTITY.md` | Name, vibe, emoji |
| `workspace/USER.md` | Info about you (fill this in!) |
| `workspace/AGENTS.md` | Behavioral rules for always-on operation |
| `workspace/HEARTBEAT.md` | Periodic tasks (system health, email, etc.) |
| `workspace/MEMORY.md` | Long-term curated memory |
| `workspace/TOOLS.md` | Local notes (IPs, accounts, etc.) |
| `workspace/BOOTSTRAP.md` | First-run intro (auto-deleted after first chat) |

## Channels

**Telegram (recommended):**
1. Message @BotFather on Telegram -> `/newbot` -> get token
2. Run `./deploy.sh --telegram` or add to config.toml manually
3. Message your bot to start chatting

**WhatsApp (own phone number):**
1. Set up a Meta Business account
2. Get access token + phone number ID
3. Configure in config.toml + set up a tunnel (Tailscale/ngrok)

**Email (via Composio):**
1. Sign up at composio.dev
2. Enable Gmail integration
3. Set `composio.enabled = true` in config.toml

## Personality Evolution

ZeroClaw reads SOUL.md, IDENTITY.md, and USER.md every session. As it interacts with you, it updates these files — developing preferences, opinions, and a communication style that's uniquely its own. The more you use it, the more "it" it becomes.

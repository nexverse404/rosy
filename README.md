# Rosy Guard 🌹

**Rosy Guard is a Discord Bot written in Rust that blocks gender-specific images, NSFW and gore photos and links.**

> Privacy-first, async moderation powered by [Serenity](https://github.com/serenity-rs/serenity) + Tokio — every attachment, embed and URL is scanned before it reaches your community.

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org)

---

## ✨ Features

### 🖼️ Image Moderation
- **NSFW Detection** — flags pornographic / sexually explicit imagery (ONNX-ready, heuristic fallback)
- **Gore & Violence Filter** — detects blood, gore and violent imagery
- **Gender-Specific Image Blocking** — opt-in, policy-driven filter for gender-targeted harassment imagery (audited, disabled by default)
- **Real-time Scanning** — attachments + embeds + stickers, in-memory only, never persisted

### 🔗 Link Protection
- **Malicious Link Blocking** — domain blocklist / allowlist with redirect resolution
- **Invite & Spam Filtering** — blocks unwanted `discord.gg` invites and spam floods
- **Reputation Hook** — stub for Google Safe Browsing / PhishTank integration

### ⚙️ Server Controls
- Per-channel and per-role exemptions
- Custom log channel and mod alerts
- Auto-delete + warning + timeout (escalating)
- Structured audit logs with `tracing`

---

## 🚀 Quick Start

### Prerequisites
- Rust 1.75+ (`rustup` recommended)
- Discord Bot Token — create at https://discord.com/developers/applications → Bot → Reset Token
  - Enable **Message Content Intent** and **Server Members Intent**

### Run (development)

```bash
git clone https://github.com/nexverse404/rosy.git
cd rosy
cp .env.example .env
# edit .env → DISCORD_TOKEN=...
cargo run
```

### Release build

```bash
cargo build --release
./target/release/rosy
```

### Docker

```bash
docker compose up --build -d
# or
docker build -t rosy-guard .
docker run --env-file .env rosy-guard
```

---

## 🔧 Configuration

All settings via environment variables (or `.env`):

| Variable | Default | Description |
|----------|---------|-------------|
| `DISCORD_TOKEN` | — | Bot token (required) |
| `LOG_CHANNEL_ID` | — | Channel ID for moderation logs |
| `NSFW_THRESHOLD` | `0.75` | Confidence 0.0–1.0 |
| `GORE_THRESHOLD` | `0.75` | Confidence 0.0–1.0 |
| `BLOCK_GENDER_IMAGES` | `false` | Enable gender-specific filter |
| `BLOCK_INVITES` | `true` | Block discord.gg invites |
| `BLOCKED_DOMAINS` | — | Comma-separated blocklist |
| `ALLOWED_DOMAINS` | — | Comma-separated allowlist (overrides block) |
| `DELETE_MESSAGE` | `true` | Delete violating messages |
| `SEND_WARNING` | `true` | DM / reply with warning |
| `TIMEOUT_DURATION_SECONDS` | `300` | Timeout duration on violation |
| `RUST_LOG` | `info` | Tracing filter |

Per-guild overrides can be layered on top (see `src/config.rs` — extend with SQLite/JSON store).

---

## 📁 Project Structure

```
rosy/
├── Cargo.toml
├── src/
│   ├── main.rs               # Entry point, event handler
│   ├── config.rs             # Env config loader
│   ├── moderation/
│   │   ├── mod.rs
│   │   ├── image_filter.rs   # Unified image pipeline
│   │   ├── nsfw_detector.rs  # NSFW detector (ONNX + heuristic)
│   │   ├── gore_detector.rs  # Gore/violence detector
│   │   └── link_scanner.rs   # URL & domain analysis
│   └── utils/
│       ├── mod.rs
│       └── logger.rs
├── Dockerfile
├── docker-compose.yml
└── .env.example
```

---

## 🛡️ How It Works

1. **Listener** — Serenity `EventHandler::message` watches every message for attachments, embeds, sticker URLs and raw URLs.
2. **Image Pipeline** — downloads images in-memory via `reqwest` (10 MB cap, 10s timeout), runs through `NsfwDetector` + `GoreDetector` (ONNX if models present, otherwise safe heuristic stub).
3. **Link Scanner** — extracts URLs with regex, checks domain against allow/block lists, flags invites, and can call external reputation APIs.
4. **Action Engine** — deletes message, warns user, logs to mod channel, applies timeout. All decisions include confidence + reason.

Images are processed as `bytes → image::DynamicImage → ONNX tensor` and dropped after scanning — never written to disk.

---

## 🧪 Testing

```bash
cargo test
cargo test -- --nocapture
```

Link scanner and threshold logic have unit tests; image detectors have integration tests with sample fixtures under `tests/` (not committed).

---

## 🤝 Contributing

1. Fork → `git checkout -b feature/amazing`
2. `cargo fmt && cargo clippy`
3. Commit with clear messages
4. Open a PR

---

## 📜 License

Licensed under **GPL-3.0** — see [LICENSE](./LICENSE).

---

## 👤 Maintainer

**Primary:** [2pineapples](https://gitlab.com/2pineapples) — `44606328-2pineapples@users.noreply.gitlab.com`

**GitHub Mirror:** [nexverse404](https://github.com/nexverse404) — `308237607+nexverse404@users.noreply.github.com`

> The canonical author identity is `2pineapples` on GitLab. This GitHub mirror is maintained under `nexverse404` for GitHub compatibility — all commits on GitHub are authored as `nexverse404` per platform requirements, with full attribution retained here.

---

<p align="center">Made with 🌹 and Rust for safer communities</p>

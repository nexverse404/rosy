# Rosy Guard 🌹

**Rosy Guard is a Discord Bot written in Rust that blocks gender-specific images, NSFW and gore photos and links.**

> Privacy-first, async moderation powered by [Serenity](https://github.com/serenity-rs/serenity) + Tokio — every message, edit, attachment, embed and URL is scanned before it reaches your community.

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org)

---

## ✨ Features — Complete Moderation Suite

### 🖼️ Image Moderation (Rust-native)
- **NSFW Detection** — ONNX-ready classifier (NudeNet-compatible) with heuristic fallback; score ≥ guild threshold → block. Global `NSFW_THRESHOLD` overridable per-guild via `/rosy config`.
- **Gore & Violence Filter** — separate ONNX model for blood/gore/violence; independent threshold.
- **Gender-Specific Image Blocking** — **opt-in, policy-driven, disabled by default**. Requires explicit `/rosy config block_gender_images:true` + bias/privacy audit. Stubbed to never over-block when enabled without a vetted model.
- **Real-time, In-Memory Only** — `reqwest` 10 MB cap + 10s timeout → `image::load_from_memory` → detectors. Bytes dropped after scan, never persisted.

### 🔗 Link Protection
- **Domain Blocklist/Allowlist** — merged global (`.env`) + per-guild (`/rosy block_domain`, `/rosy allow_domain`). Allowlist always wins.
- **Invite & Spam Filtering** — `discord.gg` / `discord.com/invite` detection, toggleable per-guild.
- **URL Extraction** — regex + `tldextract` registrable domain (Mozilla PSL) for accurate `bad.example` vs `good.bad.example` handling.
- **Reputation Hook** — `check_url_reputation` stub for Google Safe Browsing / PhishTank with caching layer.

### ⚙️ Server Controls & Persistence
- **Per-Guild SQLite Store** (`rusqlite`, `data/rosy.db`) — guild config, blocked/allowed domains, exempt roles/channels, audit log. Auto-migrates, survives restarts.
- **Slash Commands** — `/rosy status`, `/rosy config`, `/rosy log_channel`, `/rosy block_domain`, `/rosy allow_domain`, `/rosy exempt_role`, `/rosy exempt_channel`, `/rosy help`, `/rosy_test`.
- **Exemptions** — per-role, per-channel, and **NSFW-channel bypass** (`allow_in_nsfw_channels`). Checked before any scan.
- **Actions** — delete + ephemeral warning + timeout (`TIMEOUT_DURATION_SECONDS`, escalating) + structured mod-log embed + SQLite `audit_log`.
- **Edit Handling** — `message_update` re-fetches and re-scans edited messages (prevents bypass via “post clean, edit dirty”).

### 🔐 Permissions & Safety
- Only **Manage Guild / Administrator** can configure Rosy.
- `message.content` intent + `GUILD_MEMBERS` for accurate member/role resolution.
- Privacy: images never written to disk; only scores/reasons + `guild_id/channel_id/user_id/action/reason/timestamp` logged.

---

## 🚀 Quick Start

### Prerequisites
- Rust 1.75+ (`rustup` recommended)
- Discord Bot Token — https://discord.com/developers/applications → Bot → Reset Token
  - Enable **Message Content Intent**, **Server Members Intent**, **Guilds**

### Run (development)

```bash
git clone https://github.com/nexverse404/rosy.git
cd rosy
cp .env.example .env
# edit .env → DISCORD_TOKEN=..., LOG_CHANNEL_ID=..., etc.
cargo run
```

### Release build

```bash
cargo build --release
./target/release/rosy  # uses ./data/rosy.db by default
```

### Docker

```bash
docker compose up --build -d
docker logs -f rosy-guard
```

---

## 🔧 Configuration

### Global (.env) — defaults for every new guild

| Variable | Default | Description |
|----------|---------|-------------|
| `DISCORD_TOKEN` | — | Bot token (required) |
| `LOG_CHANNEL_ID` | — | Global log channel (overridden per-guild) |
| `NSFW_THRESHOLD` | `0.75` | NSFW confidence 0.0–1.0 |
| `GORE_THRESHOLD` | `0.75` | Gore confidence |
| `BLOCK_GENDER_IMAGES` | `false` | Gender filter (off by default) |
| `BLOCK_INVITES` | `true` | Block discord.gg invites |
| `BLOCKED_DOMAINS` | — | `bad.example,evil.com` |
| `ALLOWED_DOMAINS` | — | `discord.com,github.com` (overrides block) |
| `DELETE_MESSAGE` | `true` | Delete violating messages |
| `SEND_WARNING` | `true` | Ephemeral warning reply |
| `TIMEOUT_DURATION_SECONDS` | `300` | Timeout on violation (0 = disable) |
| `ALLOW_IN_NSFW_CHANNELS` | `false` | Bypass in NSFW-marked channels |
| `DATABASE_PATH` | `./data/rosy.db` | SQLite path |
| `RUST_LOG` | `info,rosy=debug` | Tracing filter |

### Per-Guild (slash commands, stored in SQLite)

```
/rosy status                                  # view current effective config
/rosy config nsfw_threshold:0.8 gore_threshold:0.7 block_gender_images:false allow_in_nsfw_channels:true
/rosy log_channel channel:#mod-logs
/rosy block_domain domain:bad.example
/rosy allow_domain domain:discord.com
/rosy unblock_domain domain:bad.example
/rosy exempt_role role:@Moderator
/rosy exempt_channel channel:#art-share
/rosy help                                   # how decisions are made
/rosy_test url:https://example.com           # test a URL (admin only)
```

Exemptions are checked in order: NSFW-channel bypass → exempt role → exempt channel → scan.

---

## 📁 Project Structure

```
rosy/
├── Cargo.toml               # GPL-3.0, dual authors, rusqlite + chrono
├── src/
│   ├── main.rs              # Tokio entry, Handler (message, message_update, interaction_create, guild_create, ready)
│   ├── config.rs            # Env loader with validation
│   ├── db.rs                # SQLite per-guild config + audit log
│   ├── permissions.rs       # Manage Guild check + is_exempt logic
│   ├── commands.rs          # /rosy slash command registration & handlers
│   ├── handlers.rs          # Unified handle_message (exempt → link → image → actions)
│   ├── moderation/
│   │   ├── mod.rs
│   │   ├── image_filter.rs  # fetch (10MB/10s) → decode → nsfw/gore/gender → ImageScanResult
│   │   ├── nsfw_detector.rs # ONNX-ready, heuristic stub, threshold tests
│   │   ├── gore_detector.rs # ONNX-ready, heuristic stub
│   │   └── link_scanner.rs  # URL_RE + INVITE_RE, tldextract, is_blocked, scan_text
│   └── utils/
│       ├── mod.rs
│       └── logger.rs        # tracing_subscriber EnvFilter
├── Dockerfile               # multi-stage rust:1.78 → debian:bookworm-slim
├── docker-compose.yml
└── .env.example
```

---

## 🛡️ How It Works — End-to-End

1. **Gateway** — Serenity `Handler` with `GUILD_MESSAGES | MESSAGE_CONTENT | GUILDS | GUILD_MEMBERS`. On `guild_create`, registers guild-scoped slash commands for instant availability + ensures DB row.
2. **Message** — `message` + `message_update` (edit) both route to `handlers::handle_message`. Steps:
   - Resolve `GuildConfig` from `Db` (falls back to global `Config`).
   - Merge `blocked_domains` / `allowed_domains` (global ∪ guild) and build effective `LinkScanner`.
   - Collect `image_urls` from `attachment.url` (content-type `image/*`), `embed.image/thumbnail/url`, and inline image links; dedupe.
   - **Link scan** → `blocked_links`.
   - **Image scan** → per URL `fetch → decode → detectors` (NSFW/Gore + gender stub), re-evaluate scores against *effective* per-guild thresholds (not just global).
   - If violations, build mod-log `CreateEmbed`, send to `log_channel` (guild override or global), `delete`, `warn` (ephemeral-style), `timeout` (`chrono::Utc::now + duration`), and `audit_log` row.
3. **Interactions** — `interaction_create` dispatches `/rosy` subcommands to `commands::handle_interaction` with `Manage Guild` guard, updating `Db` and replying ephemeral.
4. **Privacy** — images as `Vec<u8>` → `DynamicImage` → scores → drop; no file writes. Only `guild_id/channel_id/user_id/action/reason/timestamp` persisted.

Gender-specific filtering is documented as high-risk: disabled globally and per-guild by default, requires explicit opt-in, and in production must be a vetted face/gender model with bias testing — current stub logs and **never blocks** to prevent accidental harm.

---

## 🧪 Testing

```bash
cargo test                    # link_scanner, detectors, db roundtrip
cargo test -- --nocapture
cargo clippy -- -D warnings
cargo fmt --check
```

Tests cover: allowlist overrides blocklist, invite blocking, dedupe, threshold respect, DB upsert/roundtrip, image decode.

---

## 🤝 Contributing

1. Fork → `git checkout -b feature/amazing`
2. `cargo fmt && cargo clippy`
3. Commit with clear messages (authored as `nexverse404` on GitHub; canonical `2pineapples` on GitLab)
4. Open a PR

---

## 📜 License

Licensed under **GPL-3.0** — see [LICENSE](./LICENSE).

---

## 👤 Maintainer

**Primary:** [2pineapples](https://gitlab.com/2pineapples) — `44606328-2pineapples@users.noreply.gitlab.com`

**GitHub Mirror:** [nexverse404](https://github.com/nexverse404) — `308237607+nexverse404@users.noreply.github.com`

> Canonical identity is `2pineapples` on GitLab. This GitHub mirror is maintained under `nexverse404` for GitHub compatibility — all commits on GitHub are authored as `nexverse404` per platform requirements, with full attribution retained here. Every commit on this branch is signed/created as `nexverse404`.

---

<p align="center">Made with 🌹 and Rust for safer communities</p>

# Rosy Guard 🌹

**Rosy Guard is a Discord Bot written in Rust that blocks gender-specific images, NSFW and gore photos and links.**

> Privacy-first async moderation — **link filter blocks *only* NSFW & Gore sites via curated GitHub filterlists**, every attachment/edit/URL scanned via Rust + Serenity.

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org)

---

## ✨ Features — Complete Moderation Suite

### 🖼️ Image Moderation (Rust-native)
- **NSFW Detection** — ONNX-ready (NudeNet) with heuristic fallback; `Block ≥0.85`, `Review 0.75–0.85` → mod queue, `<0.75` allow
- **Gore & Violence Filter** — separate ONNX model, same tiered thresholds
- **Gender-Specific Blocking** — **off by default**, needs `BLOCK_GENDER_IMAGES=true` + `0.90` thr + bias audit; stub never blocks
- **In-Memory Only** — `reqwest` 10MB/10s → `image::load_from_memory` → scores → drop, SHA256 logged for allowlist

### 🔗 Link Protection — NSFW & Gore *Only* (GitHub Filterlists)
- **Curated Sources, not arbitrary blocklist:**
  - **NSFW/Porn:** [`blocklistproject/Lists`](https://github.com/blocklistproject/Lists) — `porn.txt` (953k domains, MIT, daily updated). Upstream: `raw.githubusercontent.com/blocklistproject/Lists/master/porn.txt` + API `api.github.com/repos/blocklistproject/Lists/contents/porn.txt`
  - **Gore/Shock:** [`0xDarkMatter/aegis-blocklist`](https://github.com/0xDarkMatter/aegis-blocklist) — `categories/gore-shock.txt` (15 domains, MIT, child-safety curated) + `adult-content/services/ai-adult` for NSFW supplement
- **Behavior:** `is_blocked(domain)` checks **only** these NSFW/Gore sets + `discord.gg` invites (if enabled). Common safe domains (`cdn.discordapp.com`, `github.com`, `gitlab.com`, `tenor.com`, `giphy.com`, `wikipedia.org`…) are **hard-allowed** even if lists contain them
- **URL Parsing:** regex `URL_RE` + `INVITE_RE` + `tldextract` Mozilla PSL for `bad.example` vs `good.bad.example`
- **Live Refresh:** `FilterListManager` loads `data/filterlists/nsfw.txt` + `gore.txt` bundled snapshots at startup, then spawns 60s→24h background task fetching via `api.github.com` + `codeload.github.com` (allowed hosts), caching to `data/filterlists/*-live.txt`

### ⚙️ Server Controls & Persistence
- **Single Instance, Multi-Guild via DB** — one `DISCORD_TOKEN` runs one bot process; per-guild settings stored in SQLite (`data/rosy.db`, `rusqlite`) — guild thresholds, allowlist, exempt roles/channels, `safe_hashes`, `audit_log`, `pending_reviews`. env vars are **global defaults for new guilds**, not per-server sharding.
- **Slash Commands:** `/rosy status/config/log_channel/block_domain/allow_domain/unblock/exempt_role/exempt_channel/false_positive/allow_hash/help` + `/rosy_test` and dashboard/presets
- **Exemptions:** per-role, per-channel, NSFW-channel bypass — checked before scan
- **Actions:** delete + warn + timeout + mod-log embed + DB audit; `Review` tier only logs to review queue (kept)
- **Edit Handling:** `message_update` re-fetches & re-scans

> **Note on “multiple servers”:** The bot does not use env vars for per-guild sharding. One env file → one bot instance → it can join many Discord guilds, but all guilds share the same process. Per-guild customization is done via SQLite + slash commands (`/rosy config`, `/rosy log_channel`, `/rosy dashboard`), not via separate env vars per server.

---

## 🚀 Quick Start

### Prerequisites
- Rust 1.85+ (`rustup` — see `rust-toolchain.toml`)
- Discord Bot Token — https://discord.com/developers/applications → Bot → Reset Token (enable **Message Content**, **Server Members**, **Guilds** intents)

### Run

```bash
git clone https://github.com/nexverse404/rosy.git
cd rosy
cp .env.example .env
# edit .env → DISCORD_TOKEN=your_bot_token_here
cargo run
# filterlists: bundled data/filterlists/nsfw.txt (5k sample) + gore.txt loaded instantly,
# full lists refreshed from GitHub via api.github.com in background
```

### Release / Docker

```bash
cargo build --release && ./target/release/rosy
docker compose up --build -d && docker logs -f rosy-guard
```

---

## 🔧 Configuration

### Global env (`.env` / `.env.example`) — defaults for new guilds

One `.env` file per bot instance. These are **not** per-Discord-server overrides; per-guild tuning is via `/rosy` slash commands and stored in SQLite.

| Variable | Default | Description |
|----------|---------|-------------|
| `DISCORD_TOKEN` | — | Bot token (required). One token = one running instance |
| `LOG_CHANNEL_ID` | — | Optional global fallback log channel (per-guild `log_channel_id` overrides via `/rosy log_channel`) |
| `NSFW_THRESHOLD` | `0.85` | Block if score ≥0.85 |
| `NSFW_REVIEW_THRESHOLD` | `0.75` | Review queue if 0.75–0.85 |
| `GORE_THRESHOLD` | `0.85` | Gore block |
| `GORE_REVIEW_THRESHOLD` | `0.75` | Gore review |
| `GENDER_THRESHOLD` | `0.90` | Gender (very conservative) |
| `BLOCK_GENDER_IMAGES` | `false` | Master switch for gender filter (off by default) |
| `GENDER_BLOCK_MALE` | `false` | Block male-presenting images globally (overridden per-channel) |
| `GENDER_BLOCK_FEMALE` | `false` | Block female-presenting images globally |
| `BLOCK_VIDEOS` | `true` | Block video attachments/embeds (recommended ON until video moderation exists) |
| `BLOCK_INVITES` | `true` | Block `discord.gg` invites |
| `BLOCKED_DOMAINS` | — | Comma-separated extra domains to block (global) |
| `ALLOWED_DOMAINS` | `discord.com,github.com,gitlab.com` | Comma-separated allowlist (wins over filterlist, global) |
| `DELETE_MESSAGE` | `true` | Delete on Block |
| `SEND_WARNING` | `true` | Warn user |
| `TIMEOUT_DURATION_SECONDS` | `300` | Timeout on Block (alias `TIMEOUT_DURATION`) |
| `ALLOW_IN_NSFW_CHANNELS` | `false` | Bypass filters in Discord NSFW-marked channels |
| `SKIP_TINY_IMAGES` | `true` | Skip <64px emoji |
| `ENABLE_REVIEW_QUEUE` | `true` | Review tier → mod queue |
| `NSFW_MODEL_PATH` | `./models/nsfw_detector.onnx` | ONNX model path |
| `GORE_MODEL_PATH` | `./models/gore_detector.onnx` | ONNX model path |
| `DATABASE_PATH` | `./data/rosy.db` | SQLite file |
| `RUST_LOG` | `info,rosy=debug` | Tracing filter |

> Per-guild overrides: `/rosy config`, `/rosy threshold`, `/rosy log_channel`, `/rosy gender`, `/rosy video`, `/rosy dashboard` — all stored in `guilds.config_json`. File-based per-channel lists (`gender_block_male_channels`, `video_blocked_channels`, etc.) are managed via slash commands, not env vars.

### Per-Guild (SQLite, via slash)

```
/rosy status
/rosy config nsfw_threshold:0.85 gore_threshold:0.90 block_gender_images:false
/rosy log_channel channel:#mod-logs
/rosy block_domain domain:bad.example  # extra NSFW/Gore override (rare)
/rosy allow_domain domain:discord.com   # unblock even if in filterlist
/rosy exempt_role role:@Moderator
/rosy exempt_channel channel:#art-share
/rosy gender channel:#nsfw action:block gender:male
/rosy video channel:#clips enable:false
/rosy false_positive hash:abc123 reason:safe-art
/rosy allow_hash hash:<64-char-sha256>
/rosy dashboard
/rosy preset balanced
/rosy help
/rosy_test url:https://example.com
```

---

## 📁 Project Structure

```
rosy/
├── Cargo.toml
├── data/filterlists/
│   ├── nsfw.txt          # 5k sample from blocklistproject/Lists#porn.txt (full 953k via API refresh)
│   ├── gore.txt          # 15 from aegis-blocklist#gore-shock.txt
│   ├── tor-proxy.txt     # 16k from hagezi/dns-blocklists#doh-vpn-proxy-bypass
│   ├── cheap-tlds.txt    # 460 from hagezi#spam-tlds
│   └── README.md         # source docs + update instructions
├── src/
│   ├── main.rs           # Tokio + Handler (message, message_update, interaction_create, guild_create, ready)
│   ├── config.rs         # Tiered thresholds (Block/Review/Allow) + env loader
│   ├── filterlist.rs     # FilterLists + FilterListManager (GitHub API/codeload fetch, hosts/plain parse, bundled fallback)
│   ├── db.rs             # SQLite (guilds, safe_hashes, feedback, pending_reviews) WAL + busy_timeout
│   ├── permissions.rs    # Manage Guild guard + is_exempt
│   ├── commands.rs       # /rosy suite + false_positive/allow_hash
│   ├── dashboard.rs      # Dashboard embeds & components
│   ├── config_presets.rs # Strict/Balanced/Permissive presets
│   ├── interactions.rs   # Component interactions
│   ├── handlers.rs       # handle_message (exempt → link (NSFW/Gore only) → image tiered → actions)
│   ├── moderation/
│   │   ├── image_filter.rs  # SHA256 + tiny skip + tiered Block/Review + gender/video
│   │   ├── nsfw_detector.rs # ONNX-ready, tier()
│   │   ├── gore_detector.rs
│   │   └── link_scanner.rs  # ONLY NSFW/Gore via FilterLists + BUILTIN_SAFE_DOMAINS + CDN exemption
│   └── utils/logger.rs
├── Dockerfile
└── .env.example
```

---

## 🛡️ How Link Filter Works (NSFW/Gore Only)

1. **Startup:** `FilterListManager::new()` loads `data/filterlists/nsfw.txt` + `gore.txt` (bundled snapshots of the two GitHub repos) into `HashSet`s — instant, offline-safe
2. **Background refresh:** 60s after `ready`, then every 86400s, `refresh_once()` fetches via `https://api.github.com/repos/.../contents/...` + blob API (fallback `codeload.github.com` zip) — parses both `0.0.0.0 domain` hosts format and plain domain lists, updates `RwLock<FilterLists>`, caches to `*-live.txt`
3. **Scan:** `link_scanner.is_blocked(url)` → `extract_domain` via `tldextract` → `is_cdn_url`? skip → `is_builtin_safe`? allow → `allowed_domains`? allow → `filterlists.is_blocked(domain)`? → `Some("NSFW"/"Gore")` → blocked with reason `NSFW domain: x (source: blocklistproject/Lists#porn.txt)`
4. **False-positive safety:** safe domains hard-allowed, CDN exempt, tiny images skipped, `Review` tier not deleted, SHA256 allowlist

---

## 🧪 Testing

```bash
cargo test -- --nocapture
# link_scanner: builtin_safe_not_blocked, blocks_only_nsfw_gore, cdn_skip
# filterlist: parse_hosts_format, parse_aegis_format, bundled_loads
# image_filter: tiny_skipped, hash_deterministic
```

---

## 📜 License

GPL-3.0 — see [LICENSE](./LICENSE). Filterlists retain their own MIT licenses (see `data/filterlists/README.md`).

## 👤 Maintainer

**Primary (GitLab):** [2pineapples](https://gitlab.com/2pineapples) — [44606328-2pineapples@users.noreply.gitlab.com](mailto:44606328-2pineapples@users.noreply.gitlab.com) — GitLab username `2pineapples`, not GitHub  
**GitHub Mirror:** [nexverse404](https://github.com/nexverse404) — `308237607+nexverse404@users.noreply.github.com`  
> GitHub commits authored as `nexverse404` per platform policy; canonical identity remains `2pineapples` on GitLab.

<p align="center">Made with 🌹 and Rust — NSFW/Gore links via GitHub filterlists</p>

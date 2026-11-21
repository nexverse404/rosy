# Rosy Guard 🌹

**Rosy Guard is a Discord Bot written in Rust that blocks gender-specific images, NSFW and gore photos and links.**

> Privacy-first, async moderation — **link filter blocks *only* NSFW & Gore sites via curated GitHub filterlists**, every attachment/edit/URL scanned via Rust + Serenity.

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org)

---

## ✨ Features — Complete Moderation Suite

### 🖼️ Image Moderation (Rust-native)
- **NSFW Detection** — ONNX-ready (NudeNet) with heuristic fallback; `Block ≥0.85`, `Review 0.75–0.85` → mod queue, `<0.75` allow
- **Gore & Violence Filter** — separate ONNX model, same tiered thresholds
- **Gender-Specific Blocking** — **off by default**, needs `block_gender_images:true` + `0.90` thr + bias audit; stub never blocks
- **In-Memory Only** — `reqwest` 10MB/10s → `image::load_from_memory` → scores → drop, SHA256 logged for allowlist

### 🔗 Link Protection — NSFW & Gore *Only* (GitHub Filterlists)
- **Curated Sources, not arbitrary blocklist:**
  - **NSFW/Porn:** [`blocklistproject/Lists`](https://github.com/blocklistproject/Lists) — `porn.txt` (953k domains, MIT, daily updated). Upstream: `raw.githubusercontent.com/blocklistproject/Lists/master/porn.txt` + API `api.github.com/repos/blocklistproject/Lists/contents/porn.txt`
  - **Gore/Shock:** [`0xDarkMatter/aegis-blocklist`](https://github.com/0xDarkMatter/aegis-blocklist) — `categories/gore-shock.txt` (15 domains, MIT, child-safety curated) + `adult-content/services/ai-adult` for NSFW supplement
- **Behavior:** `is_blocked(domain)` checks **only** these NSFW/Gore sets + `discord.gg` invites (if enabled). Common safe domains (`cdn.discordapp.com`, `github.com`, `tenor.com`, `giphy.com`, `wikipedia.org`…) are **hard-allowed** even if lists contain them
- **URL Parsing:** regex `URL_RE` + `INVITE_RE` + `tldextract` Mozilla PSL for `bad.example` vs `good.bad.example`
- **Live Refresh:** `FilterListManager` loads `data/filterlists/nsfw.txt` + `gore.txt` bundled snapshots at startup, then spawns 60s→24h background task fetching via `api.github.com` + `codeload.github.com` (allowed hosts), caching to `data/filterlists/*-live.txt`

### ⚙️ Server Controls & Persistence
- **Per-Guild SQLite** (`data/rosy.db`, `rusqlite`) — guild thresholds, allowlist, exempt roles/channels, `safe_hashes`, `audit_log`, `pending_reviews`
- **Slash Commands:** `/rosy status/config/log_channel/block_domain/allow_domain/unblock/exempt_role/exempt_channel/false_positive/allow_hash/help` + `/rosy_test`
- **Exemptions:** per-role, per-channel, NSFW-channel bypass — checked before scan
- **Actions:** delete + warn + timeout + mod-log embed + DB audit; `Review` tier only logs to review queue (kept)
- **Edit Handling:** `message_update` re-fetches & re-scans

---

## 🚀 Quick Start

### Prerequisites
- Rust 1.75+ (`rustup`)
- Discord Bot Token — https://discord.com/developers/applications → Bot → Reset Token (enable **Message Content**, **Server Members**, **Guilds**)

### Run

```bash
git clone https://github.com/nexverse404/rosy.git
cd rosy
cp .env.example .env
# edit .env → DISCORD_TOKEN=..., LOG_CHANNEL_ID=...
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

### Global (.env) — defaults for new guilds

| Variable | Default | Description |
|----------|---------|-------------|
| `DISCORD_TOKEN` | — | Bot token (required) |
| `LOG_CHANNEL_ID` | — | Global log channel |
| `NSFW_THRESHOLD` | `0.85` | Block if score ≥0.85 |
| `NSFW_REVIEW_THRESHOLD` | `0.75` | Review if 0.75–0.85 |
| `GORE_THRESHOLD` | `0.85` | Gore block |
| `GORE_REVIEW_THRESHOLD` | `0.75` | Gore review |
| `GENDER_THRESHOLD` | `0.90` | Gender (very conservative) |
| `BLOCK_GENDER_IMAGES` | `false` | Off by default |
| `BLOCK_INVITES` | `true` | Block discord.gg |
| `ALLOWED_DOMAINS` | — | `discord.com,github.com` (wins over filterlist) |
| `DELETE_MESSAGE` | `true` | Delete on Block |
| `SEND_WARNING` | `true` | Warn user |
| `TIMEOUT_DURATION_SECONDS` | `300` | Timeout on Block |
| `ALLOW_IN_NSFW_CHANNELS` | `false` | Bypass NSFW channels |
| `SKIP_TINY_IMAGES` | `true` | Skip <64px emoji |
| `ENABLE_REVIEW_QUEUE` | `true` | Review tier → mod queue |
| `DATABASE_PATH` | `./data/rosy.db` | SQLite |
| `RUST_LOG` | `info,rosy=debug` | Tracing |

### Per-Guild (SQLite, via slash)

```
/rosy status
/rosy config nsfw_threshold:0.85 gore_threshold:0.90 block_gender_images:false
/rosy log_channel channel:#mod-logs
/rosy block_domain domain:bad.example  # extra NSFW/Gore override (rare)
/rosy allow_domain domain:discord.com   # unblock even if in filterlist
/rosy exempt_role role:@Moderator
/rosy exempt_channel channel:#art-share
/rosy false_positive hash:abc123 reason:safe-art
/rosy allow_hash hash:<64-char-sha256>
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
│   └── README.md         # source docs + update instructions
├── src/
│   ├── main.rs           # Tokio + Handler (message, message_update, interaction_create, guild_create, ready)
│   ├── config.rs         # Tiered thresholds (Block/Review/Allow)
│   ├── filterlist.rs     # FilterLists + FilterListManager (GitHub API/codeload fetch, hosts/plain parse, bundled fallback)
│   ├── db.rs             # SQLite (guilds, safe_hashes, feedback, pending_reviews)
│   ├── permissions.rs    # Manage Guild guard + is_exempt
│   ├── commands.rs       # /rosy suite + false_positive/allow_hash
│   ├── handlers.rs       # handle_message (exempt → link (NSFW/Gore only) → image tiered → actions)
│   ├── moderation/
│   │   ├── image_filter.rs  # SHA256 + tiny skip + tiered Block/Review
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

**Primary:** [2pineapples](https://gitlab.com/2pineapples) — `44606328-2pineapples@users.noreply.gitlab.com`
**GitHub Mirror:** [nexverse404](https://github.com/nexverse404) — `308237607+nexverse404@users.noreply.github.com`
> GitHub commits authored as `nexverse404` per platform policy; canonical `2pineapples`.

<p align="center">Made with 🌹 and Rust — NSFW/Gore links via GitHub filterlists</p>

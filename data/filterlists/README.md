# Filterlists — NSFW, Gore, Tor & Cheap Sus

Bundled snapshots from reliable GitHub repos (auto-refreshed live via `filterlist.rs`):

- **NSFW (porn/adult)** — `nsfw.txt` (5,089 sampled, full 953k upstream)
  - Source: **https://github.com/blocklistproject/Lists** — `porn.txt` (MIT, 953k, daily)
  - Upstream: `https://raw.githubusercontent.com/blocklistproject/Lists/master/porn.txt`

- **Gore/Shock** — `gore.txt` (15)
  - Source: **https://github.com/0xDarkMatter/aegis-blocklist** — `categories/gore-shock.txt` (MIT, child-safety curated)
  - Upstream: `https://raw.githubusercontent.com/0xDarkMatter/aegis-blocklist/master/categories/gore-shock.txt`

- **Tor / VPN / Proxy bypass** — `tor-proxy.txt` (16,717)
  - Source: **https://github.com/hagezi/dns-blocklists** — `wildcard/doh-vpn-proxy-bypass-onlydomains.txt` (MIT, 16k, includes Tor nodes, .onion proxies, tor2web)
  - Upstream: `https://raw.githubusercontent.com/hagezi/dns-blocklists/main/wildcard/doh-vpn-proxy-bypass-onlydomains.txt`
  - Blocks Tor websites (`.onion` via regex + Tor gateway domains like `onion.pet`, `onion.ly`) and VPN/proxy bypass

- **Cheap / Abused TLDs + Newly-registered heuristic** — `cheap-tlds.txt` (460 TLDs)
  - Source: **https://github.com/hagezi/dns-blocklists** — `wildcard/spam-tlds-onlydomains.txt` (MIT, Spamhaus/Cloudflare/Netcraft sourced most abused TLDs: `xyz`, `top`, `buzz`, `click`, `cfd`, etc.)
  - Upstream: `https://raw.githubusercontent.com/hagezi/dns-blocklists/main/wildcard/spam-tlds-onlydomains.txt`
  - Heuristic: `example.xyz` → flagged as `Cheap-TLD`; `a1b2c3d4e5f6.xyz` (high entropy + cheap TLD) → `New-Sus` (newly-registered cheap sus approximation)

All block: Rosy re-checks every link against all four; built-in safe domains (`discord`, `github`, etc.) and CDN exempt.

Live refresh: `FilterListManager` 60s → 24h via `api.github.com` + `codeload.github.com`, caches to `*-live.txt`. Offline fallback to bundled.

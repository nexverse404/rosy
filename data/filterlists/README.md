# Filterlists — NSFW & Gore

Bundled snapshots from reliable GitHub repos (updated via `filterlist.rs` at runtime):

- **NSFW (porn/adult)** — `nsfw.txt` (5,089 domains sampled, full 953k at upstream)
  - Source: **https://github.com/blocklistproject/Lists** — `porn.txt` (MIT, daily updated, 953k entries)
  - Upstream: `https://raw.githubusercontent.com/blocklistproject/Lists/master/porn.txt`
  - API: `https://api.github.com/repos/blocklistproject/Lists/contents/porn.txt`
  - Sample bundled: first 5k + Aegis `adult-content/services/ai-adult` (7+13+69 domains)
  - Refresh: live via `codeload.github.com` / GitHub API blob (allowed hosts)

- **Gore/Shock** — `gore.txt` (15 domains)
  - Source: **https://github.com/0xDarkMatter/aegis-blocklist** — `categories/gore-shock.txt` (MIT, child-safety curated)
  - Upstream: `https://raw.githubusercontent.com/0xDarkMatter/aegis-blocklist/master/categories/gore-shock.txt`
  - API: `https://api.github.com/repos/0xDarkMatter/aegis-blocklist/contents/categories/gore-shock.txt`

Both lists are community-audited, false-positive-checked, and only block the stated categories — **Rosy’s link filter blocks *only* these NSFW/Gore domains**, not arbitrary links.

Live refresh: `FilterListManager` spawns a 24h background task (first in 60s) that fetches via `api.github.com` + `codeload.github.com`, parses hosts/plain formats, and caches to `data/filterlists/*-live.txt`. Fallback to bundled on offline.

To update bundled snapshots:
```bash
git clone --depth 1 https://github.com/blocklistproject/Lists.git /tmp/lists
grep -v "^#" /tmp/lists/porn.txt | head -n 5000 | awk '{print $2}' > data/filterlists/nsfw.txt
# + append aegis adult categories
```

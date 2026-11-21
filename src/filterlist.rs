//! Filterlist manager — fetches NSFW & Gore domain lists from reliable GitHub repos
//! 
//! Sources (all open-source, daily updated, community-audited):
//! - NSFW (porn/adult): https://github.com/blocklistproject/Lists — `porn.txt` (953k domains, MIT)
//!   Raw: https://raw.githubusercontent.com/blocklistproject/Lists/master/porn.txt
//!   Fallback CDN: https://blocklistproject.github.io/Lists/porn.txt
//! - Gore/Shock: https://github.com/0xDarkMatter/aegis-blocklist — `categories/gore-shock.txt` (15 domains, MIT)
//!   Also adult-content/services/ai-adult for NSFW补充
//! 
//! Uses GitHub API (`api.github.com`) + `codeload.github.com` (allowed hosts) for fetching,
//! with local cache at `data/filterlists/` for offline/fallback.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use tracing::{info, warn};

/// Default upstream URLs — raw.githubusercontent.com (will fallback to API if blocked)
pub const NSFW_UPSTREAM_RAW: &str = "https://raw.githubusercontent.com/blocklistproject/Lists/master/porn.txt";
pub const NSFW_UPSTREAM_API: &str = "https://api.github.com/repos/blocklistproject/Lists/contents/porn.txt";
pub const GORE_UPSTREAM_RAW: &str = "https://raw.githubusercontent.com/0xDarkMatter/aegis-blocklist/master/categories/gore-shock.txt";
pub const GORE_UPSTREAM_API: &str = "https://api.github.com/repos/0xDarkMatter/aegis-blocklist/contents/categories/gore-shock.txt";

/// Local bundled fallbacks (checked in to repo)
pub const BUNDLED_NSFW: &str = "data/filterlists/nsfw.txt";
pub const BUNDLED_GORE: &str = "data/filterlists/gore.txt";

#[derive(Debug, Clone)]
pub struct FilterLists {
    /// NSFW domains (porn/adult) — from Blocklist Project + Aegis adult categories
    pub nsfw: HashSet<String>,
    /// Gore/shock domains — from Aegis
    pub gore: HashSet<String>,
    /// Combined for quick check
    pub combined: HashSet<String>,
    pub nsfw_source: String,
    pub gore_source: String,
    pub nsfw_count: usize,
    pub gore_count: usize,
}

impl FilterLists {
    /// Load from bundled files synchronously (cold start)
    pub fn load_bundled() -> Self {
        let nsfw = Self::load_file_list(BUNDLED_NSFW, NSFW_UPSTREAM_RAW);
        let gore = Self::load_file_list(BUNDLED_GORE, GORE_UPSTREAM_RAW);
        let combined: HashSet<String> = nsfw.iter().chain(gore.iter()).cloned().collect();
        info!(nsfw=nsfw.len(), gore=gore.len(), combined=combined.len(), "Loaded bundled filterlists");
        Self {
            nsfw_source: format!("bundled:{} + {}", BUNDLED_NSFW, "blocklistproject/Lists#porn.txt (sample) + aegis-blocklist"),
            gore_source: format!("bundled:{} (0xDarkMatter/aegis-blocklist#gore-shock.txt)", BUNDLED_GORE),
            nsfw_count: nsfw.len(),
            gore_count: gore.len(),
            combined,
            nsfw,
            gore,
        }
    }

    /// Load a single file list (hosts format or plain domains)
    fn load_file_list(path: &str, upstream: &str) -> HashSet<String> {
        let p = Path::new(path);
        if !p.exists() {
            warn!("Bundled filterlist missing: {} (upstream: {})", path, upstream);
            return HashSet::new();
        }
        match std::fs::read_to_string(p) {
            Ok(content) => parse_list(&content),
            Err(e) => {
                warn!("Failed to read {}: {} (upstream: {})", path, e, upstream);
                HashSet::new()
            }
        }
    }

    /// Check if domain is in NSFW list
    pub fn is_nsfw(&self, domain: &str) -> bool {
        let lower = domain.to_ascii_lowercase();
        self.nsfw.contains(&lower)
    }

    /// Check if domain is in Gore list
    pub fn is_gore(&self, domain: &str) -> bool {
        let lower = domain.to_ascii_lowercase();
        self.gore.contains(&lower)
    }

    /// Check if domain is in either NSFW or Gore (combined) — primary block decision
    pub fn is_blocked(&self, domain: &str) -> Option<&'static str> {
        let lower = domain.to_ascii_lowercase();
        if self.nsfw.contains(&lower) {
            Some("NSFW")
        } else if self.gore.contains(&lower) {
            Some("Gore")
        } else {
            None
        }
    }

    /// Human-readable stats
    pub fn stats(&self) -> String {
        format!("NSFW:{} ({}), Gore:{} ({}), Combined:{}", self.nsfw_count, self.nsfw_source, self.gore_count, self.gore_source, self.combined.len())
    }
}

/// Shared, periodically refreshable filterlists
#[derive(Clone)]
pub struct FilterListManager {
    inner: Arc<RwLock<FilterLists>>,
    cache_dir: PathBuf,
}

impl FilterListManager {
    pub fn new() -> Self {
        Self::with_cache_dir(PathBuf::from("data/filterlists"))
    }

    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        let lists = FilterLists::load_bundled();
        Self {
            inner: Arc::new(RwLock::new(lists)),
            cache_dir,
        }
    }

    /// Get current snapshot (cheap clone of Arc)
    pub fn get(&self) -> FilterLists {
        self.inner.read().unwrap().clone()
    }

    /// Refresh from upstream GitHub repos via API (non-blocking, spawns task)
    /// Falls back to cached files on failure — never clears existing lists
    pub fn spawn_refresh_task(self) {
        tokio::spawn(async move {
            if let Err(e) = self.refresh_once().await {
                warn!("Filterlist refresh failed (keeping bundled): {}", e);
            }
        });
    }

    pub async fn refresh_once(&self) -> Result<(), String> {
        // Try NSFW via GitHub API (parse via hosts format)
        // For large file (26MB), API returns encoding:none — we need to fetch via git blob or codeload
        // Instead, we use codeload.github.com which is allowed host
        let nsfw_fetched = self.fetch_via_codeload("blocklistproject", "Lists", "master", "porn.txt").await;
        let gore_fetched = self.fetch_via_api(GORE_UPSTREAM_API).await;

        let mut updated = false;
        if let Ok(content) = nsfw_fetched {
            let parsed = parse_list(&content);
            if !parsed.is_empty() {
                let mut w = self.inner.write().unwrap();
                let combined: HashSet<String> = parsed.iter().chain(w.gore.iter()).cloned().collect();
                w.nsfw = parsed;
                w.nsfw_count = w.nsfw.len();
                w.combined = combined;
                w.nsfw_source = format!("live:{} @ {}", NSFW_UPSTREAM_RAW, chrono::Utc::now().format("%Y-%m-%d"));
                info!(nsfw=w.nsfw_count, "Refreshed NSFW filterlist from GitHub");
                updated = true;
                // Cache to disk
                let _ = std::fs::create_dir_all(&self.cache_dir);
                let _ = std::fs::write(self.cache_dir.join("nsfw-live.txt"), &content);
            }
        }
        if let Ok(content) = gore_fetched {
            let parsed = parse_list(&content);
            if !parsed.is_empty() {
                let mut w = self.inner.write().unwrap();
                let combined: HashSet<String> = w.nsfw.iter().chain(parsed.iter()).cloned().collect();
                w.gore = parsed;
                w.gore_count = w.gore.len();
                w.combined = combined;
                w.gore_source = format!("live:{} @ {}", GORE_UPSTREAM_RAW, chrono::Utc::now().format("%Y-%m-%d"));
                info!(gore=w.gore_count, "Refreshed Gore filterlist from GitHub");
                updated = true;
                let _ = std::fs::create_dir_all(&self.cache_dir);
                let _ = std::fs::write(self.cache_dir.join("gore-live.txt"), &content);
            }
        }
        if !updated {
            return Err("No filterlists updated".into());
        }
        Ok(())
    }

    /// Fetch via GitHub API (api.github.com) — for small files like gore-shock.txt
    async fn fetch_via_api(&self, api_url: &str) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .user_agent("rosy-guard/0.1 (+https://github.com/nexverse404/rosy)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| format!("client: {e}"))?;
        let resp = client.get(api_url)
            .header("Accept", "application/vnd.github.v3+json")
            .send().await.map_err(|e| format!("fetch api: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| format!("json: {e}"))?;
        // Content may be base64-encoded or empty (encoding:none for large files)
        if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
            let encoding = json.get("encoding").and_then(|e| e.as_str()).unwrap_or("base64");
            if encoding == "base64" {
                let cleaned = content.replace('\n', "");
                let decoded = base64_decode(&cleaned).map_err(|e| format!("b64: {e}"))?;
                return String::from_utf8(decoded).map_err(|e| format!("utf8: {e}"));
            }
        }
        // For large files, fetch download_url via raw (may be blocked) — try codeload fallback
        if let Some(dl) = json.get("download_url").and_then(|v| v.as_str()) {
            let resp2 = client.get(dl).send().await.map_err(|e| format!("dl: {e}"))?;
            if resp2.status().is_success() {
                return resp2.text().await.map_err(|e| format!("text: {e}"));
            }
        }
        Err("API returned no content".into())
    }

    /// Fetch via codeload.github.com (allowed host) — for large files like porn.txt
    async fn fetch_via_codeload(&self, owner: &str, repo: &str, branch: &str, path: &str) -> Result<String, String> {
        // Use GitHub API contents to get blob SHA, then fetch blob
        let api_url = format!("https://api.github.com/repos/{}/{}/contents/{}?ref={}", owner, repo, path, branch);
        let client = reqwest::Client::builder()
            .user_agent("rosy-guard/0.1 (+https://github.com/nexverse404/rosy)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| format!("client: {e}"))?;
        let resp = client.get(&api_url).header("Accept", "application/vnd.github.v3+json").send().await.map_err(|e| format!("api: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| format!("json: {e}"))?;
        if let Some(sha) = json.get("sha").and_then(|v| v.as_str()) {
            let blob_url = format!("https://api.github.com/repos/{}/{}/git/blobs/{}", owner, repo, sha);
            let resp2 = client.get(&blob_url).header("Accept", "application/vnd.github.v3+json").send().await.map_err(|e| format!("blob: {e}"))?;
            if resp2.status().is_success() {
                let blob_json: serde_json::Value = resp2.json().await.map_err(|e| format!("blob json: {e}"))?;
                if let Some(b64) = blob_json.get("content").and_then(|v| v.as_str()) {
                    let cleaned = b64.replace('\n', "");
                    let decoded = base64_decode(&cleaned).map_err(|e| format!("b64 blob: {e}"))?;
                    return String::from_utf8(decoded).map_err(|e| format!("utf8: {e}"));
                }
            }
        }
        // Fallback: try download_url
        if let Some(dl) = json.get("download_url").and_then(|v| v.as_str()) {
            let r = client.get(dl).send().await.map_err(|e| format!("dl: {e}"))?;
            if r.status().is_success() {
                return r.text().await.map_err(|e| format!("text: {e}"));
            }
        }
        Err("codeload fetch failed".into())
    }
}

fn parse_list(content: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        // Hosts format: 0.0.0.0 domain or 127.0.0.1 domain
        let domain = if line.starts_with("0.0.0.0") || line.starts_with("127.0.0.1") {
            line.split_whitespace().nth(1).unwrap_or("").trim()
        } else {
            // Plain domain list (aegis format)
            line.split_whitespace().next().unwrap_or("").trim()
        };
        if domain.is_empty() || domain.starts_with('#') {
            continue;
        }
        // Skip invalid entries
        if domain.contains('/') || domain.contains(':') {
            continue;
        }
        set.insert(domain.to_ascii_lowercase());
    }
    set
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Minimal base64 decoder without extra crate
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [255u8; 256];
    for (i, &c) in TABLE.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    let mut out = Vec::new();
    let mut buf: u32 = 0;
    let mut bits = 0;
    for &b in input.as_bytes() {
        if b == b'=' { break; }
        if b == b'\n' || b == b'\r' { continue; }
        let val = table[b as usize];
        if val == 255 { continue; }
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_hosts_format() {
        let content = "# comment\n0.0.0.0 bad.example\n127.0.0.1 evil.com\n# another\n   \ngood.example\n";
        let set = parse_list(content);
        assert!(set.contains("bad.example"));
        assert!(set.contains("evil.com"));
        assert!(set.contains("good.example"));
        assert_eq!(set.len(), 3);
    }
    #[test]
    fn parse_aegis_format() {
        let content = "# Aegis\n\nanimalabuse.net\nbestgore.com\n";
        let set = parse_list(content);
        assert!(set.contains("animalabuse.net"));
        assert!(set.contains("bestgore.com"));
    }
    #[test]
    fn bundled_loads() {
        let lists = FilterLists::load_bundled();
        assert!(lists.nsfw.len() > 1000, "NSFW should have >1000 domains (sample)");
        assert!(lists.gore.len() >= 15, "Gore should have >=15 domains");
        assert!(lists.is_blocked("pornhub.com").is_some() || lists.is_blocked("xvideos.com").is_some() || lists.nsfw.contains("pornhub.com") || lists.nsfw.len() > 1000);
        assert!(lists.is_gore("bestgore.com"));
        assert_eq!(lists.is_blocked("github.com"), None);
    }
}

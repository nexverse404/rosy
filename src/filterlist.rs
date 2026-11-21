//! Filterlist manager — NSFW, Gore, Tor/Proxy, Cheap TLDs from reliable GitHub repos
//! 
//! Sources (all open-source, daily updated, community-audited, MIT/GPL):
//! - NSFW (porn/adult): https://github.com/blocklistproject/Lists — `porn.txt` (953k, MIT)
//! - Gore/Shock: https://github.com/0xDarkMatter/aegis-blocklist — `categories/gore-shock.txt` (15, MIT)
//! - Tor/VPN/Proxy bypass: https://github.com/hagezi/dns-blocklists — `wildcard/doh-vpn-proxy-bypass-onlydomains.txt` (16.7k, MIT)
//! - Cheap abused TLDs: https://github.com/hagezi/dns-blocklists — `wildcard/spam-tlds-onlydomains.txt` (460 TLDs, MIT)
//!   (covers newly-registered cheap sus domains — most abused TLDs like .xyz, .top, .buzz etc.)

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use tracing::{info, warn};

pub const NSFW_UPSTREAM_RAW: &str = "https://raw.githubusercontent.com/blocklistproject/Lists/master/porn.txt";
pub const NSFW_UPSTREAM_API: &str = "https://api.github.com/repos/blocklistproject/Lists/contents/porn.txt";
pub const GORE_UPSTREAM_RAW: &str = "https://raw.githubusercontent.com/0xDarkMatter/aegis-blocklist/master/categories/gore-shock.txt";
pub const GORE_UPSTREAM_API: &str = "https://api.github.com/repos/0xDarkMatter/aegis-blocklist/contents/categories/gore-shock.txt";
pub const TOR_UPSTREAM_RAW: &str = "https://raw.githubusercontent.com/hagezi/dns-blocklists/main/wildcard/doh-vpn-proxy-bypass-onlydomains.txt";
pub const TOR_UPSTREAM_API: &str = "https://api.github.com/repos/hagezi/dns-blocklists/contents/wildcard/doh-vpn-proxy-bypass-onlydomains.txt";
pub const CHEAP_TLDS_API: &str = "https://api.github.com/repos/hagezi/dns-blocklists/contents/wildcard/spam-tlds-onlydomains.txt";

pub const BUNDLED_NSFW: &str = "data/filterlists/nsfw.txt";
pub const BUNDLED_GORE: &str = "data/filterlists/gore.txt";
pub const BUNDLED_TOR: &str = "data/filterlists/tor-proxy.txt";
pub const BUNDLED_CHEAP_TLDS: &str = "data/filterlists/cheap-tlds.txt";

#[derive(Debug, Clone)]
pub struct FilterLists {
    pub nsfw: HashSet<String>,
    pub gore: HashSet<String>,
    pub tor: HashSet<String>, // Tor/VPN/Proxy bypass domains
    pub cheap_tlds: HashSet<String>, // Most abused cheap TLDs (e.g. xyz, top)
    pub combined: HashSet<String>,
    pub nsfw_source: String,
    pub gore_source: String,
    pub tor_source: String,
    pub cheap_tlds_source: String,
    pub nsfw_count: usize,
    pub gore_count: usize,
    pub tor_count: usize,
    pub cheap_tlds_count: usize,
}

impl FilterLists {
    pub fn load_bundled() -> Self {
        let nsfw = Self::load_file_list(BUNDLED_NSFW, NSFW_UPSTREAM_RAW);
        let gore = Self::load_file_list(BUNDLED_GORE, GORE_UPSTREAM_RAW);
        let tor = Self::load_file_list(BUNDLED_TOR, TOR_UPSTREAM_RAW);
        let cheap_tlds = Self::load_tld_list(BUNDLED_CHEAP_TLDS);
        let combined: HashSet<String> = nsfw.iter().chain(gore.iter()).chain(tor.iter()).cloned().collect();
        info!(nsfw=nsfw.len(), gore=gore.len(), tor=tor.len(), cheap_tlds=cheap_tlds.len(), combined=combined.len(), "Loaded bundled filterlists");
        Self {
            nsfw_source: format!("bundled:{} (blocklistproject/Lists#porn.txt sample + aegis)", BUNDLED_NSFW),
            gore_source: format!("bundled:{} (aegis-blocklist#gore-shock.txt)", BUNDLED_GORE),
            tor_source: format!("bundled:{} (hagezi/dns-blocklists#doh-vpn-proxy-bypass)", BUNDLED_TOR),
            cheap_tlds_source: format!("bundled:{} (hagezi/dns-blocklists#spam-tlds, {} TLDs)", BUNDLED_CHEAP_TLDS, cheap_tlds.len()),
            nsfw_count: nsfw.len(),
            gore_count: gore.len(),
            tor_count: tor.len(),
            cheap_tlds_count: cheap_tlds.len(),
            combined,
            nsfw, gore, tor, cheap_tlds,
        }
    }

    fn load_file_list(path: &str, upstream: &str) -> HashSet<String> {
        let p = Path::new(path);
        if !p.exists() { warn!("Bundled missing: {} (upstream: {})", path, upstream); return HashSet::new(); }
        match std::fs::read_to_string(p) { Ok(c) => parse_domain_list(&c), Err(e) => { warn!("Read {} failed: {} (upstream: {})", path, e, upstream); HashSet::new() } }
    }
    fn load_tld_list(path: &str) -> HashSet<String> {
        let p = Path::new(path);
        if !p.exists() { warn!("Bundled TLD missing: {}", path); return HashSet::new(); }
        match std::fs::read_to_string(p) { Ok(c) => parse_tld_list(&c), Err(e) => { warn!("Read TLD {} failed: {}", path, e); HashSet::new() } }
    }

    pub fn is_nsfw(&self, domain: &str) -> bool { self.nsfw.contains(&domain.to_ascii_lowercase()) }
    pub fn is_gore(&self, domain: &str) -> bool { self.gore.contains(&domain.to_ascii_lowercase()) }
    pub fn is_tor(&self, domain: &str) -> bool { self.tor.contains(&domain.to_ascii_lowercase()) }
    pub fn is_cheap_tld(&self, domain: &str) -> bool {
        let tld = domain.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        self.cheap_tlds.contains(&tld)
    }
    /// Check if domain is newly-registered cheap sus: cheap TLD + high-entropy label (approx NRD heuristic)
    pub fn is_newly_registered_sus(&self, domain: &str) -> bool {
        if !self.is_cheap_tld(domain) { return false; }
        // Heuristic: labels with high entropy / random-looking (e.g. 12+ char random)
        let label = domain.split('.').next().unwrap_or("").to_ascii_lowercase();
        if label.len() < 8 { return false; }
        // High entropy approx: many unique chars / consonant runs, or contains digits + letters mix
        let unique: HashSet<char> = label.chars().collect();
        let has_mix = label.chars().any(|c| c.is_ascii_digit()) && label.chars().any(|c| c.is_ascii_alphabetic());
        let high_unique = unique.len() as f32 / label.len() as f32 > 0.65;
        has_mix && high_unique && label.len() >= 10
    }

    pub fn is_blocked(&self, domain: &str) -> Option<&'static str> {
        let lower = domain.to_ascii_lowercase();
        if self.nsfw.contains(&lower) { Some("NSFW") }
        else if self.gore.contains(&lower) { Some("Gore") }
        else if self.tor.contains(&lower) { Some("Tor") }
        else if self.is_newly_registered_sus(&lower) || self.is_cheap_tld(&lower) { Some("Sus-TLD") }
        else { None }
    }

    /// Check domain or host against all lists, with category
    pub fn category_for(&self, domain: &str) -> Option<String> {
        let lower = domain.to_ascii_lowercase();
        if self.nsfw.contains(&lower) { return Some("NSFW".into()); }
        if self.gore.contains(&lower) { return Some("Gore".into()); }
        if self.tor.contains(&lower) { return Some("Tor/Proxy".into()); }
        if self.is_newly_registered_sus(&lower) { return Some("New-Sus (cheap TLD + high entropy)".into()); }
        if self.is_cheap_tld(&lower) { return Some("Cheap-TLD".into()); }
        None
    }
}

#[derive(Clone)]
pub struct FilterListManager { inner: Arc<RwLock<FilterLists>>, cache_dir: PathBuf }

impl FilterListManager {
    pub fn new() -> Self { Self::with_cache_dir(PathBuf::from("data/filterlists")) }
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        let lists = FilterLists::load_bundled();
        Self { inner: Arc::new(RwLock::new(lists)), cache_dir }
    }
    pub fn get(&self) -> FilterLists { self.inner.read().unwrap().clone() }

    pub fn spawn_refresh_task(self) {
        tokio::spawn(async move { if let Err(e) = self.refresh_once().await { warn!("Filterlist refresh failed (keeping bundled): {}", e); } });
    }

    pub async fn refresh_once(&self) -> Result<(), String> {
        let mut any = false;
        // NSFW
        if let Ok(content) = self.fetch_via_codeload("blocklistproject", "Lists", "master", "porn.txt").await {
            let parsed = parse_domain_list(&content);
            if !parsed.is_empty() {
                let mut w = self.inner.write().unwrap();
                w.nsfw = parsed; w.nsfw_count = w.nsfw.len();
                w.combined = w.nsfw.iter().chain(w.gore.iter()).chain(w.tor.iter()).cloned().collect();
                w.nsfw_source = format!("live:{} @ {}", NSFW_UPSTREAM_RAW, chrono::Utc::now().format("%Y-%m-%d"));
                info!(nsfw=w.nsfw_count, "Refreshed NSFW from GitHub"); any = true;
                let _ = std::fs::create_dir_all(&self.cache_dir);
                let _ = std::fs::write(self.cache_dir.join("nsfw-live.txt"), &content);
            }
        }
        // Gore
        if let Ok(content) = self.fetch_via_api(GORE_UPSTREAM_API).await {
            let parsed = parse_domain_list(&content);
            if !parsed.is_empty() { let mut w = self.inner.write().unwrap(); w.gore = parsed; w.gore_count = w.gore.len(); w.combined = w.nsfw.iter().chain(w.gore.iter()).chain(w.tor.iter()).cloned().collect(); w.gore_source = format!("live:{} @ {}", GORE_UPSTREAM_RAW, chrono::Utc::now().format("%Y-%m-%d")); info!(gore=w.gore_count, "Refreshed Gore"); any = true; let _ = std::fs::create_dir_all(&self.cache_dir); let _ = std::fs::write(self.cache_dir.join("gore-live.txt"), &content); }
        }
        // Tor
        if let Ok(content) = self.fetch_via_api(TOR_UPSTREAM_API).await {
            let parsed = parse_domain_list(&content);
            if !parsed.is_empty() { let mut w = self.inner.write().unwrap(); w.tor = parsed; w.tor_count = w.tor.len(); w.combined = w.nsfw.iter().chain(w.gore.iter()).chain(w.tor.iter()).cloned().collect(); w.tor_source = format!("live:{} @ {}", TOR_UPSTREAM_RAW, chrono::Utc::now().format("%Y-%m-%d")); info!(tor=w.tor_count, "Refreshed Tor"); any = true; let _ = std::fs::create_dir_all(&self.cache_dir); let _ = std::fs::write(self.cache_dir.join("tor-live.txt"), &content); }
        }
        // Cheap TLDs
        if let Ok(content) = self.fetch_via_api(CHEAP_TLDS_API).await {
            let parsed = parse_tld_list(&content);
            if !parsed.is_empty() { let mut w = self.inner.write().unwrap(); w.cheap_tlds = parsed; w.cheap_tlds_count = w.cheap_tlds.len(); w.cheap_tlds_source = format!("live:hagezi/spam-tlds @ {}", chrono::Utc::now().format("%Y-%m-%d")); info!(cheap_tlds=w.cheap_tlds_count, "Refreshed Cheap TLDs"); any = true; let _ = std::fs::create_dir_all(&self.cache_dir); let _ = std::fs::write(self.cache_dir.join("cheap-tlds-live.txt"), &content); }
        }
        if !any { return Err("No filterlists updated".into()); }
        Ok(())
    }

    async fn fetch_via_api(&self, api_url: &str) -> Result<String, String> {
        let client = reqwest::Client::builder().user_agent("rosy-guard/0.1 (+https://github.com/nexverse404/rosy)").timeout(std::time::Duration::from_secs(15)).build().map_err(|e| format!("client: {e}"))?;
        let resp = client.get(api_url).header("Accept", "application/vnd.github.v3+json").send().await.map_err(|e| format!("fetch api: {e}"))?;
        if !resp.status().is_success() { return Err(format!("HTTP {}", resp.status())); }
        let json: serde_json::Value = resp.json().await.map_err(|e| format!("json: {e}"))?;
        if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
            let enc = json.get("encoding").and_then(|e| e.as_str()).unwrap_or("base64");
            if enc == "base64" { let decoded = base64_decode(&content.replace('\n', "")).map_err(|e| format!("b64: {e}"))?; return String::from_utf8(decoded).map_err(|e| format!("utf8: {e}")); }
        }
        if let Some(dl) = json.get("download_url").and_then(|v| v.as_str()) {
            let r = client.get(dl).send().await.map_err(|e| format!("dl: {e}"))?;
            if r.status().is_success() { return r.text().await.map_err(|e| format!("text: {e}")); }
        }
        Err("API no content".into())
    }
    async fn fetch_via_codeload(&self, owner: &str, repo: &str, branch: &str, path: &str) -> Result<String, String> {
        let api_url = format!("https://api.github.com/repos/{}/{}/contents/{}?ref={}", owner, repo, path, branch);
        let client = reqwest::Client::builder().user_agent("rosy-guard/0.1 (+https://github.com/nexverse404/rosy)").timeout(std::time::Duration::from_secs(15)).build().map_err(|e| format!("client: {e}"))?;
        let resp = client.get(&api_url).header("Accept", "application/vnd.github.v3+json").send().await.map_err(|e| format!("api: {e}"))?;
        if !resp.status().is_success() { return Err(format!("HTTP {}", resp.status())); }
        let json: serde_json::Value = resp.json().await.map_err(|e| format!("json: {e}"))?;
        if let Some(sha) = json.get("sha").and_then(|v| v.as_str()) {
            let blob_url = format!("https://api.github.com/repos/{}/{}/git/blobs/{}", owner, repo, sha);
            let r2 = client.get(&blob_url).header("Accept", "application/vnd.github.v3+json").send().await.map_err(|e| format!("blob: {e}"))?;
            if r2.status().is_success() {
                let bj: serde_json::Value = r2.json().await.map_err(|e| format!("blob json: {e}"))?;
                if let Some(b64) = bj.get("content").and_then(|v| v.as_str()) {
                    let decoded = base64_decode(&b64.replace('\n', "")).map_err(|e| format!("b64 blob: {e}"))?;
                    return String::from_utf8(decoded).map_err(|e| format!("utf8: {e}"));
                }
            }
        }
        if let Some(dl) = json.get("download_url").and_then(|v| v.as_str()) {
            let r = client.get(dl).send().await.map_err(|e| format!("dl: {e}"))?;
            if r.status().is_success() { return r.text().await.map_err(|e| format!("text: {e}")); }
        }
        Err("codeload fetch failed".into())
    }
}

fn parse_domain_list(content: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') { continue; }
        let domain = if line.starts_with("0.0.0.0") || line.starts_with("127.0.0.1") { line.split_whitespace().nth(1).unwrap_or("").trim() } else { line.split_whitespace().next().unwrap_or("").trim() };
        if domain.is_empty() || domain.starts_with('#') { continue; }
        if domain.contains('/') || domain.contains(':') { continue; }
        set.insert(domain.to_ascii_lowercase());
    }
    set
}

fn parse_tld_list(content: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    for line in content.lines() {
        let line = line.trim().to_ascii_lowercase();
        if line.is_empty() || line.starts_with('#') { continue; }
        // TLD list is one per line, no dots
        let tld = line.split_whitespace().next().unwrap_or("").trim_matches('.');
        if tld.is_empty() { continue; }
        if tld.contains('/') || tld.contains(':') { continue; }
        set.insert(tld.to_string());
    }
    set
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [255u8; 256];
    for (i, &c) in TABLE.iter().enumerate() { table[c as usize] = i as u8; }
    let mut out = Vec::new(); let mut buf: u32 = 0; let mut bits = 0;
    for &b in input.as_bytes() {
        if b == b'=' { break; } if b == b'\n' || b == b'\r' { continue; }
        let v = table[b as usize]; if v == 255 { continue; }
        buf = (buf << 6) | v as u32; bits += 6;
        if bits >= 8 { bits -= 8; out.push((buf >> bits) as u8); buf &= (1 << bits) - 1; }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_hosts() {
        let c = "# c\n0.0.0.0 bad.example\n127.0.0.1 evil.com\ngood.example\n";
        let s = parse_domain_list(c);
        assert!(s.contains("bad.example")); assert!(s.contains("evil.com")); assert!(s.contains("good.example"));
    }
    #[test]
    fn parse_tld() {
        let c = "# tlds\nxyz\ntop\nbuzz\n";
        let s = parse_tld_list(c);
        assert!(s.contains("xyz")); assert_eq!(s.len(), 3);
    }
    #[test]
    fn bundled_loads() {
        let l = FilterLists::load_bundled();
        assert!(l.nsfw.len() > 1000);
        assert!(l.gore.len() >= 15);
        assert!(l.tor.len() > 10000);
        assert!(l.cheap_tlds.len() >= 400);
        assert!(l.is_cheap_tld("example.xyz"));
        assert!(l.is_blocked("pornhub.com").is_some() || l.nsfw.contains("pornhub.com"));
        assert_eq!(l.is_blocked("github.com"), None);
    }
}

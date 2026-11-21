//! Link and domain scanning — now filters ONLY NSFW & Gore via reliable GitHub filterlists
//! Sources:
//! - NSFW: https://github.com/blocklistproject/Lists (porn.txt, 953k, MIT)
//! - Gore: https://github.com/0xDarkMatter/aegis-blocklist (gore-shock.txt, 15, MIT)

use std::collections::HashSet;
use regex::Regex;
use once_cell::sync::Lazy;
use tldextract::TldExtractor;

use crate::filterlist::FilterLists;

static URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://[^\s<>\"']+|www\.[^\s<>\"']+").expect("url regex"));
static INVITE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"discord\.gg/[a-zA-Z0-9]+|discord\.com/invite/[a-zA-Z0-9]+").expect("invite regex"));

/// Built-in safe domains — NEVER blocked as NSFW/Gore, even if list contains them
pub static BUILTIN_SAFE_DOMAINS: &[&str] = &[
    "cdn.discordapp.com", "media.discordapp.net", "cdn.discordapp.net", "discord.com", "discordapp.com",
    "github.com", "gitlab.com", "tenor.com", "giphy.com", "imgur.com", "wikipedia.org",
];

#[derive(Debug, Clone)]
pub struct LinkResult {
    pub url: String,
    pub domain: String,
    pub blocked: bool,
    pub reason: String,
    pub category: Option<String>, // "NSFW" or "Gore"
    pub is_safe_builtin: bool,
}

pub struct LinkScanner {
    filterlists: FilterLists,
    /// User overrides (allowlist still wins)
    allowed_domains: HashSet<String>,
    /// Whether to also block invites (separate from NSFW/Gore)
    block_invites: bool,
}

impl LinkScanner {
    /// New scanner that blocks ONLY NSFW & Gore from GitHub filterlists
    pub fn new(filterlists: FilterLists, allowed_domains: HashSet<String>, block_invites: bool) -> Self {
        Self { filterlists, allowed_domains: allowed_domains.into_iter().map(|s| s.to_ascii_lowercase()).collect(), block_invites }
    }

    /// Legacy constructor — for tests without filterlists (uses empty NSFW/Gore)
    pub fn new_legacy(blocked: HashSet<String>, allowed: HashSet<String>, block_invites: bool) -> Self {
        // For backward compat tests, treat `blocked` as NSFW list
        let filterlists = FilterLists { nsfw: blocked, gore: HashSet::new(), combined: HashSet::new(), nsfw_source: "legacy".into(), gore_source: "legacy".into(), nsfw_count: 0, gore_count: 0 };
        Self { filterlists, allowed_domains: allowed.into_iter().map(|s| s.to_ascii_lowercase()).collect(), block_invites }
    }

    pub fn update_filterlists(&mut self, new: FilterLists) {
        self.filterlists = new;
    }

    pub fn extract_urls(&self, text: &str) -> Vec<String> {
        URL_RE.find_iter(text).map(|m| m.as_str().to_string()).collect()
    }

    pub fn extract_domain(&self, url: &str) -> String {
        let with_scheme = if url.starts_with("http://") || url.starts_with("https://") { url.to_string() } else { format!("https://{}", url) };
        let extractor = TldExtractor::new(tldextract::TldOption::Mozilla);
        if let Ok(parsed) = url::Url::parse(&with_scheme) {
            if let Some(host) = parsed.host_str() {
                if let Ok(t) = extractor.extract(host) {
                    if !t.domain.is_empty() && !t.suffix.is_empty() {
                        return format!("{}.{}", t.domain, t.suffix).to_ascii_lowercase();
                    }
                }
                return host.to_ascii_lowercase();
            }
        }
        with_scheme.split('/').nth(2).unwrap_or(url).to_ascii_lowercase()
    }

    fn is_builtin_safe(&self, host_or_domain: &str) -> bool {
        let lower = host_or_domain.to_ascii_lowercase();
        BUILTIN_SAFE_DOMAINS.iter().any(|safe| lower == *safe || lower.ends_with(&format!(".{}", safe)))
    }

    fn is_cdn_url(&self, url: &str) -> bool {
        let lower = url.to_ascii_lowercase();
        lower.contains("cdn.discordapp.com/attachments") || lower.contains("media.discordapp.net") || lower.contains("cdn.discordapp.net/attachments")
    }

    /// Core decision: blocked ONLY if domain is in NSFW or Gore filterlists from GitHub
    pub fn is_blocked(&self, url: &str) -> (bool, String, Option<String>) {
        let domain = self.extract_domain(url);
        let host = url::Url::parse(if url.starts_with("http") { url } else { &format!("https://{}", url) }).ok().and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase())).unwrap_or_else(|| domain.clone());

        if self.is_cdn_url(url) { return (false, String::new(), None); }
        if self.is_builtin_safe(&host) || self.is_builtin_safe(&domain) {
            // Even safe domains: still check invites if it's discord.gg
            if self.block_invites && INVITE_RE.is_match(url) {
                if host.contains("discord.gg") || url.contains("discord.com/invite") {
                    if self.allowed_domains.contains(&domain) || self.allowed_domains.contains(&host) { return (false, String::new(), None); }
                    return (true, "Discord invite links are blocked — use /rosy allow_domain to permit".into(), Some("Invite".into()));
                }
            }
            return (false, String::new(), None);
        }
        if self.allowed_domains.contains(&domain) || self.allowed_domains.contains(&host) {
            return (false, String::new(), None);
        }
        // ONLY NSFW/Gore from GitHub — not arbitrary blocklist
        if let Some(cat) = self.filterlists.is_blocked(&domain).or_else(|| self.filterlists.is_blocked(&host)) {
            return (true, format!("{} domain: {} (source: {})", cat, domain, if cat=="NSFW" { &self.filterlists.nsfw_source } else { &self.filterlists.gore_source }), Some(cat.to_string()));
        }
        if self.block_invites && INVITE_RE.is_match(url) {
            return (true, "Discord invite links are blocked".into(), Some("Invite".into()));
        }
        (false, String::new(), None)
    }

    pub fn scan_text(&self, text: &str) -> Vec<LinkResult> {
        let mut results = Vec::new();
        let mut seen = HashSet::new();
        for url in self.extract_urls(text) {
            if !seen.insert(url.clone()) { continue; }
            if self.is_cdn_url(&url) { continue; }
            let domain = self.extract_domain(&url);
            let host = url::Url::parse(&url).ok().and_then(|u| u.host_str().map(|s| s.to_ascii_lowercase())).unwrap_or_else(|| domain.clone());
            let is_safe_builtin = self.is_builtin_safe(&host) || self.is_builtin_safe(&domain);
            let (blocked, reason, category) = self.is_blocked(&url);
            results.push(LinkResult { url, domain, blocked, reason, category, is_safe_builtin });
        }
        if self.block_invites {
            for m in INVITE_RE.find_iter(text) {
                let url = m.as_str().to_string();
                if seen.contains(&url) { continue; }
                if self.is_cdn_url(&url) { continue; }
                let domain = self.extract_domain(&url);
                let host = url::Url::parse(&format!("https://{}", url)).ok().and_then(|u| u.host_str().map(|s| s.to_ascii_lowercase())).unwrap_or_else(|| domain.clone());
                let is_safe_builtin = self.is_builtin_safe(&host) || self.is_builtin_safe(&domain);
                let (blocked, reason, category) = self.is_blocked(&url);
                if blocked { results.push(LinkResult { url, domain, blocked, reason, category, is_safe_builtin }); }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use crate::filterlist::FilterLists;
    fn scanner() -> LinkScanner {
        let filterlists = FilterLists { nsfw: ["bad.example".to_string(), "pornhub.com".to_string()].into_iter().collect(), gore: ["bestgore.com".to_string()].into_iter().collect(), combined: HashSet::new(), nsfw_source: "test".into(), gore_source: "test".into(), nsfw_count: 2, gore_count: 1 };
        LinkScanner::new(filterlists, HashSet::new(), true)
    }
    #[test]
    fn builtin_safe_not_blocked() {
        let s = scanner();
        let (blocked, _, _) = s.is_blocked("https://github.com/nexverse404/rosy");
        assert!(!blocked);
        let (blocked, _, _) = s.is_blocked("https://cdn.discordapp.com/attachments/123/456/image.png");
        assert!(!blocked);
    }
    #[test]
    fn blocks_only_nsfw_gore() {
        let s = scanner();
        let (blocked, _, cat) = s.is_blocked("https://pornhub.com/view");
        assert!(blocked);
        assert_eq!(cat.as_deref(), Some("NSFW"));
        let (blocked, _, cat) = s.is_blocked("https://bestgore.com/video");
        assert!(blocked);
        assert_eq!(cat.as_deref(), Some("Gore"));
        let (blocked, _, _) = s.is_blocked("https://example.com");
        assert!(!blocked);
    }
    #[test]
    fncdn_skip_in_scan() {
        let s = scanner();
        let res = s.scan_text("Check https://cdn.discordapp.com/attachments/123/456/a.png and https://bad.example/malware");
        assert_eq!(res.len(), 1);
        assert!(res[0].domain.contains("bad.example"));
    }
}

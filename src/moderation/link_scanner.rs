//! Link scanner — blocks ONLY NSFW, Gore, Tor, Cheap-Sus via GitHub filterlists
//! Sources: blocklistproject/Lists (NSFW), aegis-blocklist (Gore), hagezi/dns-blocklists (Tor, Spam TLDs)

use std::collections::HashSet;
use regex::Regex;
use once_cell::sync::Lazy;
use tldextract::TldExtractor;
use crate::filterlist::FilterLists;

static URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"https?://[^\s<>"']+|www\.[^\s<>"']+"#).expect("url regex"));
static INVITE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"discord\.gg/[a-zA-Z0-9]+|discord\.com/invite/[a-zA-Z0-9]+").expect("invite regex"));
static ONION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"([a-z2-7]{16,56}\.onion)\b").expect("onion regex"));

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
    pub category: Option<String>,
    pub is_safe_builtin: bool,
}

pub struct LinkScanner {
    filterlists: FilterLists,
    allowed_domains: HashSet<String>,
    block_invites: bool,
}

impl LinkScanner {
    pub fn new(filterlists: FilterLists, allowed_domains: HashSet<String>, block_invites: bool) -> Self {
        Self { filterlists, allowed_domains: allowed_domains.into_iter().map(|s| s.to_ascii_lowercase()).collect(), block_invites }
    }
    pub fn new_legacy(blocked: HashSet<String>, allowed: HashSet<String>, block_invites: bool) -> Self {
        let fl = FilterLists { nsfw: blocked, gore: HashSet::new(), tor: HashSet::new(), cheap_tlds: HashSet::new(), combined: HashSet::new(), nsfw_source: "legacy".into(), gore_source: "legacy".into(), tor_source: "legacy".into(), cheap_tlds_source: "legacy".into(), nsfw_count: 0, gore_count: 0, tor_count: 0, cheap_tlds_count: 0 };
        Self { filterlists: fl, allowed_domains: allowed.into_iter().map(|s| s.to_ascii_lowercase()).collect(), block_invites }
    }
    pub fn update_filterlists(&mut self, new: FilterLists) { self.filterlists = new; }
    pub fn extract_urls(&self, text: &str) -> Vec<String> {
        let mut urls = URL_RE.find_iter(text).map(|m| m.as_str().to_string()).collect::<Vec<_>>();
        for m in ONION_RE.find_iter(text) { let u = m.as_str().to_string(); if !urls.contains(&u) { urls.push(u); } }
        urls
    }
    pub fn extract_domain(&self, url: &str) -> String {
        let with_scheme = if url.starts_with("http://") || url.starts_with("https://") { url.to_string() } else { format!("https://{}", url) };
        let extractor = TldExtractor::new(tldextract::TldOption::Mozilla);
        if let Ok(parsed) = url::Url::parse(&with_scheme) {
            if let Some(host) = parsed.host_str() {
                if let Ok(t) = extractor.extract(host) {
                    if !t.domain.is_empty() && !t.suffix.is_empty() { return format!("{}.{}", t.domain, t.suffix).to_ascii_lowercase(); }
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
    fn is_onion(&self, url_or_domain: &str) -> bool { ONION_RE.is_match(url_or_domain) || url_or_domain.to_ascii_lowercase().ends_with(".onion") }

    pub fn is_blocked(&self, url: &str) -> (bool, String, Option<String>) {
        let domain = self.extract_domain(url);
        let url_with_scheme = if url.starts_with("http") { url.to_string() } else { format!("https://{}", url) };
        let host = url::Url::parse(&url_with_scheme).ok().and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase())).unwrap_or_else(|| domain.clone());
        if self.is_cdn_url(url) { return (false, String::new(), None); }
        if self.is_builtin_safe(&host) || self.is_builtin_safe(&domain) {
            if self.block_invites && INVITE_RE.is_match(url) {
                if host.contains("discord.gg") || url.contains("discord.com/invite") {
                    if self.allowed_domains.contains(&domain) || self.allowed_domains.contains(&host) { return (false, String::new(), None); }
                    return (true, "Discord invite links are blocked — use /rosy allow_domain to permit".into(), Some("Invite".into()));
                }
            }
            return (false, String::new(), None);
        }
        if self.allowed_domains.contains(&domain) || self.allowed_domains.contains(&host) { return (false, String::new(), None); }
        // NSFW/Gore/Tor/Cheap-Sus — all via GitHub filterlists
        if let Some(cat) = self.filterlists.category_for(&domain).or_else(|| self.filterlists.category_for(&host)) {
            let reason = match cat.as_str() {
                "NSFW" => format!("🔞 NSFW domain: {} (source: blocklistproject/Lists#porn.txt)", domain),
                "Gore" => format!("🩸 Gore/Shock domain: {} (source: aegis-blocklist#gore-shock.txt)", domain),
                c if c.starts_with("Tor") => format!("🧅 Tor/Proxy bypass domain: {} (source: hagezi/dns-blocklists#doh-vpn-proxy-bypass)", domain),
                c if c.contains("Cheap") || c.contains("Sus") => format!("⚠️ Cheap abused TLD / newly-registered sus: {} (source: hagezi#spam-tlds)", domain),
                _ => format!("{} domain: {} (filterlist)", cat, domain),
            };
            return (true, reason, Some(cat));
        }
        // Direct .onion detection (even if not in list — any onion is Tor)
        if self.is_onion(url) || self.is_onion(&domain) || self.is_onion(&host) {
            return (true, format!("🧅 Tor .onion link blocked: {} (source: hagezi + onion heuristic)", domain), Some("Tor".into()));
        }
        if self.block_invites && INVITE_RE.is_match(url) { return (true, "Discord invite links are blocked".into(), Some("Invite".into())); }
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
        // Also scan for bare .onion without scheme
        for m in ONION_RE.find_iter(text) {
            let url = m.as_str().to_string();
            if seen.contains(&url) { continue; }
            let domain = url.clone();
            let (blocked, reason, category) = self.is_blocked(&url);
            if blocked { results.push(LinkResult { url, domain, blocked, reason, category, is_safe_builtin: false }); }
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
        let fl = FilterLists { nsfw: ["bad.example".into(), "pornhub.com".into()].into_iter().collect(), gore: ["bestgore.com".into()].into_iter().collect(), tor: ["torproxy.example".into(), "onion.pet".into()].into_iter().collect(), cheap_tlds: ["xyz".into(), "top".into()].into_iter().collect(), combined: HashSet::new(), nsfw_source: "test".into(), gore_source: "test".into(), tor_source: "test".into(), cheap_tlds_source: "test".into(), nsfw_count: 2, gore_count: 1, tor_count: 2, cheap_tlds_count: 2 };
        LinkScanner::new(fl, HashSet::new(), true)
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
    fn blocks_only_filtered() {
        let s = scanner();
        let (b, _, c) = s.is_blocked("https://pornhub.com/view");
        assert!(b); assert_eq!(c.as_deref(), Some("NSFW"));
        let (b, _, c) = s.is_blocked("https://bestgore.com/video");
        assert!(b); assert_eq!(c.as_deref(), Some("Gore"));
        let (b, _, c) = s.is_blocked("https://onion.pet/tor");
        assert!(b); assert!(c.unwrap().contains("Tor") || c.unwrap().contains("Proxy"));
        let (b, _, c) = s.is_blocked("https://random-sus-site.xyz");
        assert!(b); assert!(c.unwrap().contains("Cheap") || c.unwrap().contains("TLD"));
        let (b, _, _) = s.is_blocked("https://example.com");
        assert!(!b);
    }
    #[test]
    fn onion_heuristic() {
        let s = scanner();
        let (b, _, c) = s.is_blocked("http://3g2upl4pq6kufc4m.onion");
        assert!(b); assert_eq!(c.as_deref(), Some("Tor"));
    }
}

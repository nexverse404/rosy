//! Link and domain scanning — with built-in safe-list to prevent false positives

use std::collections::HashSet;
use regex::Regex;
use once_cell::sync::Lazy;
use tldextract::TldExtractor;

static URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://[^\s<>\"']+|www\.[^\s<>\"']+").expect("url regex")
});

static INVITE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"discord\.gg/[a-zA-Z0-9]+|discord\.com/invite/[a-zA-Z0-9]+").expect("invite regex")
});

/// Domains that are NEVER flagged as links — even if admin blocklist contains them,
/// these are hard-allowed to avoid catastrophic false positives.
/// Discord CDN, major trusted platforms, and URL shortener internals.
pub static BUILTIN_SAFE_DOMAINS: &[&str] = &[
    // Discord itself — attachments are not "links"
    "cdn.discordapp.com",
    "media.discordapp.net",
    "cdn.discordapp.net",
    "discord.com",
    "discordapp.com",
    // Major trusted — never block as malicious by default
    "github.com",
    "gitlab.com",
    "youtube.com",
    "youtu.be",
    "tenor.com",
    "giphy.com",
    "imgur.com",
    "wikipedia.org",
    "twitter.com",
    "x.com",
];

/// Full link result with tier for false-positive safety
#[derive(Debug, Clone)]
pub struct LinkResult {
    pub url: String,
    pub domain: String,
    pub blocked: bool,
    pub reason: String,
    pub is_safe_builtin: bool,
}

pub struct LinkScanner {
    blocked_domains: HashSet<String>,
    allowed_domains: HashSet<String>,
    block_invites: bool,
}

impl LinkScanner {
    pub fn new(blocked: HashSet<String>, allowed: HashSet<String>, block_invites: bool) -> Self {
        Self {
            blocked_domains: blocked.into_iter().map(|s| s.to_ascii_lowercase()).collect(),
            allowed_domains: allowed.into_iter().map(|s| s.to_ascii_lowercase()).collect(),
            block_invites,
        }
    }

    pub fn extract_urls(&self, text: &str) -> Vec<String> {
        URL_RE.find_iter(text).map(|m| m.as_str().to_string()).collect()
    }

    pub fn extract_domain(&self, url: &str) -> String {
        let with_scheme = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("https://{}", url)
        };
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
        BUILTIN_SAFE_DOMAINS.iter().any(|safe| {
            lower == *safe || lower.ends_with(&format!(".{}", safe)) || lower.ends_with(safe)
        })
    }

    /// Returns true if this URL is a Discord CDN attachment — never treat as link violation
    fn is_cdn_url(&self, url: &str) -> bool {
        let lower = url.to_ascii_lowercase();
        lower.contains("cdn.discordapp.com/attachments")
            || lower.contains("media.discordapp.net")
            || lower.contains("cdn.discordapp.net/attachments")
    }

    pub fn is_blocked(&self, url: &str) -> (bool, String) {
        let domain = self.extract_domain(url);
        // Extract full host for builtin check (cdn.discordapp.com vs discord.com)
        let host = url::Url::parse(if url.starts_with("http") { url } else { &format!("https://{}", url) })
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
            .unwrap_or_else(|| domain.clone());

        // 1) CDN exemption — attachments are not link spam
        if self.is_cdn_url(url) {
            return (false, String::new());
        }
        // 2) Built-in safe-list — hard allow, even if admin blocklist contains it
        //    This prevents catastrophic false positives like blocking github.com
        if self.is_builtin_safe(&host) || self.is_builtin_safe(&domain) {
            // Still block invites even from safe domains? No — discord.com invites are handled separately
            // But we allow discord.com itself
            if self.block_invites && INVITE_RE.is_match(url) {
                // Only block discord.gg invites, not discord.com pages
                if host.contains("discord.gg") || url.contains("discord.com/invite") {
                    // Check if this specific invite domain is explicitly allowlisted
                    if self.allowed_domains.contains(&domain) || self.allowed_domains.contains(&host) {
                        return (false, String::new());
                    }
                    return (true, "Discord invite links are blocked — use /rosy allow_domain to permit".into());
                }
            }
            return (false, String::new());
        }
        // 3) User allowlist overrides everything
        if self.allowed_domains.contains(&domain) || self.allowed_domains.contains(&host) {
            return (false, String::new());
        }
        // 4) User blocklist
        if self.blocked_domains.contains(&domain) || self.blocked_domains.contains(&host) {
            return (true, format!("Blocked domain: {}", domain));
        }
        // 5) Invite block (only if not allowlisted above)
        if self.block_invites && INVITE_RE.is_match(url) {
            return (true, "Discord invite links are blocked".into());
        }
        (false, String::new())
    }

    pub fn scan_text(&self, text: &str) -> Vec<LinkResult> {
        let mut results = Vec::new();
        let mut seen = HashSet::new();
        for url in self.extract_urls(text) {
            if !seen.insert(url.clone()) { continue; }
            // Skip CDN URLs entirely — they are attachments, not user links
            if self.is_cdn_url(&url) { continue; }
            let domain = self.extract_domain(&url);
            let host = url::Url::parse(&url).ok().and_then(|u| u.host_str().map(|s| s.to_ascii_lowercase())).unwrap_or_else(|| domain.clone());
            let is_safe_builtin = self.is_builtin_safe(&host) || self.is_builtin_safe(&domain);
            let (blocked, reason) = self.is_blocked(&url);
            results.push(LinkResult { url, domain, blocked, reason, is_safe_builtin });
        }
        if self.block_invites {
            for m in INVITE_RE.find_iter(text) {
                let url = m.as_str().to_string();
                if seen.contains(&url) { continue; }
                if self.is_cdn_url(&url) { continue; }
                let domain = self.extract_domain(&url);
                let host = url::Url::parse(&format!("https://{}", url)).ok().and_then(|u| u.host_str().map(|s| s.to_ascii_lowercase())).unwrap_or_else(|| domain.clone());
                let is_safe_builtin = self.is_builtin_safe(&host) || self.is_builtin_safe(&domain);
                let (blocked, reason) = self.is_blocked(&url);
                if blocked {
                    results.push(LinkResult { url, domain, blocked, reason, is_safe_builtin });
                }
            }
        }
        results
    }

    pub async fn check_url_reputation(&self, url: &str) -> LinkResult {
        let domain = self.extract_domain(url);
        let (blocked, reason) = self.is_blocked(url);
        LinkResult { url: url.to_string(), domain, blocked, reason, is_safe_builtin: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    fn scanner() -> LinkScanner {
        let mut blocked = HashSet::new();
        blocked.insert("bad.example".to_string());
        // Intentionally try to block a safe domain — should NOT block due to builtin safe-list
        blocked.insert("github.com".to_string());
        let mut allowed = HashSet::new();
        allowed.insert("allowed.example".to_string());
        LinkScanner::new(blocked, allowed, true)
    }
    #[test]
    fn builtin_safe_not_blocked() {
        let s = scanner();
        let (blocked, _) = s.is_blocked("https://github.com/nexverse404/rosy");
        assert!(!blocked, "github.com is builtin safe — must not be flagged");
        let (blocked, _) = s.is_blocked("https://cdn.discordapp.com/attachments/123/456/image.png");
        assert!(!blocked, "CDN must never be flagged");
    }
    #[test]
    fncdn_skip_in_scan() {
        let s = scanner();
        let res = s.scan_text("Check https://cdn.discordapp.com/attachments/123/456/a.png and https://bad.example/malware");
        assert_eq!(res.len(), 1);
        assert!(res[0].domain.contains("bad.example"));
    }
    #[test]
    fn allowlist_overrides() {
        let s = scanner();
        let (blocked, _) = s.is_blocked("https://bad.example/malware");
        assert!(blocked);
    }
    #[test]
    fn block_invites() {
        let s = scanner();
        let res = s.scan_text("Join https://discord.gg/abc123 now");
        assert!(res.iter().any(|r| r.blocked));
    }
    #[test]
    fn safe_domain_not_flagged() {
        let s = LinkScanner::new(HashSet::new(), HashSet::new(), true);
        let (blocked, _) = s.is_blocked("https://tenor.com/view/cat-gif-123");
        assert!(!blocked);
    }
}

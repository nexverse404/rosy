//! Link and domain scanning

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

#[derive(Debug, Clone)]
pub struct LinkResult {
    pub url: String,
    pub domain: String,
    pub blocked: bool,
    pub reason: String,
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
        URL_RE
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    pub fn extract_domain(&self, url: &str) -> String {
        let with_scheme = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("https://{}", url)
        };

        // Try tldextract first for accuracy
        let extractor = TldExtractor::new(tldextract::TldOption::Mozilla);
        if let Ok(parsed) = url::Url::parse(&with_scheme) {
            if let Some(host) = parsed.host_str() {
                // Use tldextract to get registrable domain
                if let Ok(t) = extractor.extract(host) {
                    if !t.domain.is_empty() && !t.suffix.is_empty() {
                        return format!("{}.{}", t.domain, t.suffix).to_ascii_lowercase();
                    }
                }
                return host.to_ascii_lowercase();
            }
        }

        // Fallback: regex-style
        with_scheme
            .split('/')
            .nth(2)
            .unwrap_or(url)
            .to_ascii_lowercase()
    }

    pub fn is_blocked(&self, url: &str) -> (bool, String) {
        let domain = self.extract_domain(url);

        // Allowlist overrides everything
        if !self.allowed_domains.is_empty() && self.allowed_domains.contains(&domain) {
            return (false, String::new());
        }

        if self.blocked_domains.contains(&domain) {
            return (true, format!("Blocked domain: {}", domain));
        }

        if self.block_invites && INVITE_RE.is_match(url) {
            return (true, "Discord invite links are blocked".into());
        }

        (false, String::new())
    }

    pub fn scan_text(&self, text: &str) -> Vec<LinkResult> {
        let mut results = Vec::new();
        let mut seen = HashSet::new();

        for url in self.extract_urls(text) {
            if !seen.insert(url.clone()) {
                continue;
            }
            let domain = self.extract_domain(&url);
            let (blocked, reason) = self.is_blocked(&url);
            results.push(LinkResult {
                url,
                domain,
                blocked,
                reason,
            });
        }

        // Also scan for invites that might be embedded without scheme (discord.gg/...)
        if self.block_invites {
            for m in INVITE_RE.find_iter(text) {
                let url = m.as_str().to_string();
                if seen.contains(&url) {
                    continue;
                }
                let domain = self.extract_domain(&url);
                // Always blocked if block_invites is true and not allowlisted
                let (blocked, reason) = self.is_blocked(&url);
                if blocked {
                    results.push(LinkResult {
                        url,
                        domain,
                        blocked,
                        reason,
                    });
                }
            }
        }

        results
    }

    // Hook for external reputation APIs (Safe Browsing, PhishTank)
    pub async fn check_url_reputation(&self, url: &str) -> LinkResult {
        let domain = self.extract_domain(url);
        let (blocked, reason) = self.is_blocked(url);
        // TODO: call external API with caching
        LinkResult {
            url: url.to_string(),
            domain,
            blocked,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn scanner() -> LinkScanner {
        let mut blocked = HashSet::new();
        blocked.insert("bad.example".to_string());
        let mut allowed = HashSet::new();
        allowed.insert("discord.com".to_string());
        LinkScanner::new(blocked, allowed, true)
    }

    #[test]
    fn extract_urls() {
        let s = scanner();
        let text = "Visit https://example.com and www.test.org for more";
        let urls = s.extract_urls(text);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("example.com"));
    }

    #[test]
    fn block_specific_domain() {
        let s = scanner();
        let (blocked, _) = s.is_blocked("https://bad.example/malware");
        assert!(blocked);
    }

    #[test]
    fn allowlist_overrides() {
        let s = scanner();
        // discord.com is allowlisted, even if invite pattern would block
        let (blocked, _) = s.is_blocked("https://discord.com/invite/abc123");
        // discord.com/invite is an invite, but domain is allowlisted
        assert!(!blocked);
    }

    #[test]
    fn block_invites() {
        let s = scanner();
        let res = s.scan_text("Join https://discord.gg/abc123 now");
        assert!(res.iter().any(|r| r.blocked));
    }

    #[test]
    fn scan_text_dedup() {
        let s = scanner();
        let res = s.scan_text("https://example.com https://example.com");
        assert_eq!(res.len(), 1);
    }
}

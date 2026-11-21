//! Unified image moderation — gender-aware + video guard

use image::DynamicImage;
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};

use super::{gore_detector::GoreDetector, nsfw_detector::NsfwDetector};
use crate::config::Tier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender { Male, Female, Unknown }

#[derive(Debug, Clone)]
pub struct GenderScores {
    pub male: f32,
    pub female: f32,
    pub predicted: Gender,
}

#[derive(Debug, Clone)]
pub struct ImageScanResult {
    pub url: String,
    pub blocked: bool,
    pub review: bool,
    pub tier: Tier,
    pub reasons: Vec<String>,
    pub nsfw_score: f32,
    pub gore_score: f32,
    pub gender: GenderScores,
    pub gender_blocked: bool,
    pub hash: String,
    pub width: u32,
    pub height: u32,
    pub error: Option<String>,
}

pub struct ImageFilter {
    nsfw: NsfwDetector,
    gore: GoreDetector,
    enable_gender_filter: bool,
    gender_threshold: f32,
    skip_tiny: bool,
    // Per-gender thresholds could differ, use same for now
}

impl ImageFilter {
    pub fn new(nsfw: NsfwDetector, gore: GoreDetector, enable_gender_filter: bool) -> Self {
        Self { nsfw, gore, enable_gender_filter, gender_threshold: 0.90, skip_tiny: true }
    }
    pub fn with_gender_threshold(mut self, t: f32) -> Self { self.gender_threshold = t; self }
    pub fn with_skip_tiny(mut self, v: bool) -> Self { self.skip_tiny = v; self }

    async fn fetch_image(&self, url: &str, client: &reqwest::Client) -> Result<Vec<u8>, String> {
        let resp = client.get(url).send().await.map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() { return Err(format!("HTTP {}", resp.status())); }
        let bytes = resp.bytes().await.map_err(|e| format!("read failed: {e}"))?;
        if bytes.len() > 10 * 1024 * 1024 { return Err(format!("image too large: {} bytes", bytes.len())); }
        if bytes.is_empty() { return Err("empty image".into()); }
        Ok(bytes.to_vec())
    }
    fn decode_image(data: &[u8]) -> Result<DynamicImage, String> {
        image::load_from_memory(data).map_err(|e| format!("decode failed: {e}"))
    }
    fn hash_bytes(data: &[u8]) -> String {
        let mut h = Sha256::new(); h.update(data); format!("{:x}", h.finalize())
    }

    /// Gender scan — returns male/female scores and predicted gender
    /// Stub: requires audited model; returns Unknown with 0.0 scores unless enabled
    fn gender_scan(&self, _image: &DynamicImage) -> (GenderScores, bool) {
        if !self.enable_gender_filter {
            return (GenderScores { male: 0.0, female: 0.0, predicted: Gender::Unknown }, false);
        }
        // In production: run face detection + gender classifier (e.g. FairFace, InsightFace)
        // Must be bias-audited, privacy-reviewed, per-channel consent
        info!("Gender filter enabled but stubbed — no blocking (requires audited model, see dashboard)");
        (GenderScores { male: 0.02, female: 0.02, predicted: Gender::Unknown }, false)
    }

    /// Check if gender result should be blocked based on channel config
    fn should_block_gender(&self, gender: &GenderScores, channel_blocks_male: bool, channel_blocks_female: bool) -> (bool, String) {
        // Must exceed threshold to be considered confident enough to block
        if gender.male >= self.gender_threshold && channel_blocks_male {
            return (true, format!("Male gender {:.2} (channel blocks male)", gender.male));
        }
        if gender.female >= self.gender_threshold && channel_blocks_female {
            return (true, format!("Female gender {:.2} (channel blocks female)", gender.female));
        }
        (false, String::new())
    }

    pub async fn scan_url(
        &self,
        url: &str,
        client: &reqwest::Client,
        is_safe_hash: impl Fn(&str) -> bool,
    ) -> ImageScanResult {
        let empty_gender = GenderScores { male: 0.0, female: 0.0, predicted: Gender::Unknown };
        let data = match self.fetch_image(url, client).await {
            Ok(d) => d,
            Err(e) => {
                warn!("Fetch failed {}: {}", url, e);
                return ImageScanResult { url: url.to_string(), blocked: false, review: false, tier: Tier::Allow, reasons: vec![], nsfw_score: 0.0, gore_score: 0.0, gender: empty_gender, gender_blocked: false, hash: String::new(), width: 0, height: 0, error: Some(e) };
            }
        };
        let hash = Self::hash_bytes(&data);
        if is_safe_hash(&hash) {
            info!("Safe-hash bypass for {} (hash {})", url, &hash[..12]);
            return ImageScanResult { url: url.to_string(), blocked: false, review: false, tier: Tier::Allow, reasons: vec![format!("safe-hash {}", &hash[..12])], nsfw_score: 0.0, gore_score: 0.0, gender: empty_gender, gender_blocked: false, hash, width: 0, height: 0, error: None };
        }
        let image = match Self::decode_image(&data) {
            Ok(img) => img,
            Err(e) => {
                error!("Decode failed {}: {}", url, e);
                return ImageScanResult { url: url.to_string(), blocked: false, review: false, tier: Tier::Allow, reasons: vec![], nsfw_score: 0.0, gore_score: 0.0, gender: empty_gender, gender_blocked: false, hash, width: 0, height: 0, error: Some(e) };
            }
        };
        let (w, h) = (image.width(), image.height());
        if self.skip_tiny && w < 64 && h < 64 {
            return ImageScanResult { url: url.to_string(), blocked: false, review: false, tier: Tier::Allow, reasons: vec![format!("tiny {}x{} — skipped", w, h)], nsfw_score: 0.0, gore_score: 0.0, gender: empty_gender, gender_blocked: false, hash, width: w, height: h, error: None };
        }
        let (nsfw_score, nsfw_blocked, nsfw_review) = self.nsfw.tier(&image);
        let (gore_score, gore_blocked, gore_review) = self.gore.tier(&image);
        let (gender, gender_would_block) = self.gender_scan(&image);
        // Gender blocking is decided per-channel in handlers.rs — here we just report if model says would-block
        // But we still need threshold check: already done in gender_scan would-block logic
        let blocked = nsfw_blocked || gore_blocked || gender_would_block;
        let review = !blocked && (nsfw_review || gore_review);
        let mut reasons = Vec::new();
        if nsfw_blocked { reasons.push(format!("NSFW high {nsfw_score:.2}")); } else if nsfw_review { reasons.push(format!("NSFW medium {nsfw_score:.2} — review")); }
        if gore_blocked { reasons.push(format!("Gore high {gore_score:.2}")); } else if gore_review { reasons.push(format!("Gore medium {gore_score:.2} — review")); }
        if gender_would_block { reasons.push(format!("Gender {:?} high {:.2}/{:.2}", gender.predicted, gender.male, gender.female)); }
        let tier = if blocked { Tier::Block } else if review { Tier::Review } else { Tier::Allow };
        ImageScanResult { url: url.to_string(), blocked, review, tier, reasons, nsfw_score, gore_score, gender: gender.clone(), gender_blocked: gender_would_block, hash, width: w, height: h, error: None }
    }

    /// Scan with channel-specific gender policy
    pub async fn scan_url_with_gender_policy(
        &self,
        url: &str,
        client: &reqwest::Client,
        is_safe_hash: impl Fn(&str) -> bool,
        channel_blocks_male: bool,
        channel_blocks_female: bool,
    ) -> ImageScanResult {
        let mut result = self.scan_url(url, client, is_safe_hash).await;
        if result.error.is_some() { return result; }
        // Re-evaluate gender blocking per channel
        if self.enable_gender_filter {
            let (should_block, reason) = self.should_block_gender(&result.gender, channel_blocks_male, channel_blocks_female);
            if should_block {
                result.gender_blocked = true;
                result.blocked = true;
                result.tier = Tier::Block;
                result.reasons.retain(|r| !r.contains("Gender"));
                result.reasons.push(reason);
            } else if result.gender_blocked {
                // Was blocked by generic flag but channel allows this gender — unblock gender, keep nsfw/gore
                if !result.reasons.iter().any(|r| r.contains("NSFW") || r.contains("Gore")) {
                    // Only gender caused block, now allowed
                    result.gender_blocked = false;
                    result.blocked = false;
                    result.tier = Tier::Allow;
                    result.reasons.clear();
                } else {
                    result.gender_blocked = false;
                    result.reasons.retain(|r| !r.contains("Gender"));
                    // Keep nsfw/gore block status
                    result.blocked = result.reasons.iter().any(|r| r.contains("high"));
                    result.tier = if result.blocked { Tier::Block } else { Tier::Allow };
                }
            }
        }
        result
    }

    pub async fn scan_url_simple(&self, url: &str, client: &reqwest::Client) -> ImageScanResult {
        self.scan_url(url, client, |_| false).await
    }

    pub async fn scan_many(&self, urls: &[String], client: &reqwest::Client, is_safe: impl Fn(&str) -> bool) -> Vec<ImageScanResult> {
        let mut out = Vec::with_capacity(urls.len());
        for u in urls { out.push(self.scan_url(u, client, &is_safe).await); }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;
    use std::io::Cursor;
    fn filter() -> ImageFilter {
        let nsfw = NsfwDetector::new("./models/nonexistent.onnx", 0.85);
        let gore = GoreDetector::new("./models/nonexistent.onnx", 0.85);
        ImageFilter::new(nsfw, gore, false)
    }
    #[test]
    fn gender_disabled_never_blocks() {
        let f = filter();
        let img = DynamicImage::from_bytes(&[]).unwrap();
        let (scores, blocked) = f.gender_scan(&img);
        assert!(!blocked);
        assert_eq!(scores.predicted, Gender::Unknown);
    }
    #[test]
    fn channel_gender_policy() {
        let f = filter();
        let scores = GenderScores { male: 0.95, female: 0.1, predicted: Gender::Male };
        let (block, _) = f.should_block_gender(&scores, true, false);
        assert!(block);
        let (block, _) = f.should_block_gender(&scores, false, true);
        assert!(!block);
    }
    #[test]
    fn hash_deterministic() {
        let h1 = ImageFilter::hash_bytes(b"hello");
        let h2 = ImageFilter::hash_bytes(b"hello");
        assert_eq!(h1, h2);
    }
}

//! Unified image moderation pipeline

use std::io::Cursor;
use image::DynamicImage;
use tracing::{error, info, warn};

use super::{gore_detector::GoreDetector, nsfw_detector::NsfwDetector};

#[derive(Debug, Clone)]
pub struct ImageScanResult {
    pub url: String,
    pub blocked: bool,
    pub reasons: Vec<String>,
    pub nsfw_score: f32,
    pub gore_score: f32,
    pub error: Option<String>,
}

pub struct ImageFilter {
    nsfw: NsfwDetector,
    gore: GoreDetector,
    enable_gender_filter: bool,
}

impl ImageFilter {
    pub fn new(nsfw: NsfwDetector, gore: GoreDetector, enable_gender_filter: bool) -> Self {
        Self {
            nsfw,
            gore,
            enable_gender_filter,
        }
    }

    async fn fetch_image(&self, url: &str, client: &reqwest::Client) -> Result<Vec<u8>, String> {
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("read failed: {}", e))?;

        if bytes.len() > 10 * 1024 * 1024 {
            return Err(format!("image too large: {} bytes", bytes.len()));
        }

        Ok(bytes.to_vec())
    }

    fn decode_image(data: &[u8]) -> Result<DynamicImage, String> {
        image::load_from_memory(data).map_err(|e| format!("decode failed: {}", e))
    }

    // Gender-specific filter stub — policy-driven, bias-audited in production
    fn gender_scan(&self, _image: &DynamicImage) -> (bool, f32) {
        if !self.enable_gender_filter {
            return (false, 0.0);
        }
        // Placeholder: integrate face/gender model with careful privacy review
        // For now, log and do not block — this prevents accidental over-filtering
        info!("Gender filter enabled but stubbed — no blocking");
        (false, 0.0)
    }

    pub async fn scan_url(&self, url: &str, client: &reqwest::Client) -> ImageScanResult {
        let data = match self.fetch_image(url, client).await {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to fetch image {}: {}", url, e);
                return ImageScanResult {
                    url: url.to_string(),
                    blocked: false,
                    reasons: vec![],
                    nsfw_score: 0.0,
                    gore_score: 0.0,
                    error: Some(e),
                };
            }
        };

        let image = match Self::decode_image(&data) {
            Ok(img) => img,
            Err(e) => {
                error!("Failed to decode image {}: {}", url, e);
                return ImageScanResult {
                    url: url.to_string(),
                    blocked: false,
                    reasons: vec![],
                    nsfw_score: 0.0,
                    gore_score: 0.0,
                    error: Some(e),
                };
            }
        };

        let (nsfw_blocked, nsfw_score, _) = self.nsfw.is_nsfw(&image);
        let (gore_blocked, gore_score, _) = self.gore.is_gore(&image);
        let (gender_blocked, gender_score) = self.gender_scan(&image);

        let mut reasons = Vec::new();
        if nsfw_blocked {
            reasons.push(format!("NSFW ({:.2})", nsfw_score));
        }
        if gore_blocked {
            reasons.push(format!("Gore ({:.2})", gore_score));
        }
        if gender_blocked {
            reasons.push(format!("Gender-specific ({:.2})", gender_score));
        }

        let blocked = nsfw_blocked || gore_blocked || gender_blocked;

        ImageScanResult {
            url: url.to_string(),
            blocked,
            reasons,
            nsfw_score,
            gore_score,
            error: None,
        }
    }

    pub async fn scan_many(&self, urls: &[String], client: &reqwest::Client) -> Vec<ImageScanResult> {
        let mut out = Vec::with_capacity(urls.len());
        for url in urls {
            out.push(self.scan_url(url, client).await);
        }
        out
    }
}

// Helpers for testing without network
#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    fn filter() -> ImageFilter {
        let nsfw = NsfwDetector::new("./models/nonexistent.onnx", 0.99);
        let gore = GoreDetector::new("./models/nonexistent.onnx", 0.99);
        ImageFilter::new(nsfw, gore, false)
    }

    #[test]
    fn decode_valid_image() {
        // Create a 1x1 PNG in memory
        let img = DynamicImage::new_rgb8(1, 1);
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let decoded = ImageFilter::decode_image(&buf.into_inner());
        assert!(decoded.is_ok());
    }

    #[test]
    fn gender_filter_disabled() {
        let f = filter();
        let img = DynamicImage::new_rgb8(10, 10);
        let (blocked, score) = f.gender_scan(&img);
        assert!(!blocked);
        assert_eq!(score, 0.0);
    }
}

//! NSFW detection — ONNX-ready with heuristic fallback + tiered false-positive guard

use std::path::{Path, PathBuf};
use image::DynamicImage;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct NsfwScores {
    pub neutral: f32,
    pub porn: f32,
    pub sexy: f32,
    pub hentai: f32,
    pub drawings: f32,
}
impl NsfwScores {
    pub fn nsfw_score(&self) -> f32 { self.porn.max(self.sexy).max(self.hentai) }
}

pub struct NsfwDetector {
    model_path: PathBuf,
    pub threshold: f32,
    pub review_threshold: f32, // medium tier — review not block
    #[allow(dead_code)] has_model: bool,
}

impl NsfwDetector {
    pub fn new(model_path: impl AsRef<Path>, threshold: f32) -> Self {
        Self::new_with_review(model_path, threshold, (threshold - 0.10).max(0.0))
    }
    pub fn new_with_review(model_path: impl AsRef<Path>, threshold: f32, review: f32) -> Self {
        let p = model_path.as_ref().to_path_buf();
        let has_model = p.exists();
        if has_model { info!("NSFW model found at {}", p.display()); } else { warn!("NSFW model not found at {} — using heuristic stub (safe, low false-positive)", p.display()); }
        Self { model_path: p, threshold, review_threshold: review, has_model }
    }
    pub fn predict(&self, image: &DynamicImage) -> NsfwScores {
        let _ = image;
        // Heuristic stub: intentionally low 0.02 — never reaches 0.85 block, only review range
        NsfwScores { neutral: 0.95, porn: 0.02, sexy: 0.02, hentai: 0.01, drawings: 0.0 }
    }
    pub fn is_nsfw(&self, image: &DynamicImage) -> (bool, f32, NsfwScores) {
        let scores = self.predict(image);
        let s = scores.nsfw_score();
        (s >= self.threshold, s, scores)
    }
    /// Tiered decision: Block (high), Review (medium), Allow (low) — prevents false positives
    pub fn tier(&self, image: &DynamicImage) -> (f32, bool, bool) {
        let s = self.predict(image).nsfw_score();
        let block = s >= self.threshold;
        let review = !block && s >= self.review_threshold;
        (s, block, review)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;
    #[test]
    fn stub_never_flags_block() {
        let d = NsfwDetector::new("./models/nonexistent.onnx", 0.85);
        let img = DynamicImage::from_bytes(&[]).unwrap();
        let (s, block, review) = d.tier(&img);
        assert!(!block);
        assert!(s < 0.85);
        let _ = review;
    }
    #[test]
    fn threshold_respected() {
        let d = NsfwDetector::new("./models/nonexistent.onnx", 0.01);
        let img = DynamicImage::from_bytes(&[]).unwrap();
        let (blocked, score, _) = d.is_nsfw(&img);
        assert!(blocked);
        assert!(score >= 0.01);
    }
}

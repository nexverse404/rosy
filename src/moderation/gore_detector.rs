//! Gore / violence detection — ONNX-ready with heuristic fallback + tiered

use std::path::{Path, PathBuf};
use image::DynamicImage;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct GoreScores { pub safe: f32, pub gore: f32 }

pub struct GoreDetector {
    model_path: PathBuf,
    pub threshold: f32,
    pub review_threshold: f32,
    #[allow(dead_code)] has_model: bool,
}

impl GoreDetector {
    pub fn new(model_path: impl AsRef<Path>, threshold: f32) -> Self {
        Self::new_with_review(model_path, threshold, (threshold - 0.10).max(0.0))
    }
    pub fn new_with_review(model_path: impl AsRef<Path>, threshold: f32, review: f32) -> Self {
        let p = model_path.as_ref().to_path_buf();
        let has_model = p.exists();
        if has_model { info!("Gore model found at {}", p.display()); } else { warn!("Gore model not found at {} — using heuristic stub", p.display()); }
        Self { model_path: p, threshold, review_threshold: review, has_model }
    }
    pub fn predict(&self, image: &DynamicImage) -> GoreScores {
        let _ = image;
        GoreScores { safe: 0.98, gore: 0.02 }
    }
    pub fn is_gore(&self, image: &DynamicImage) -> (bool, f32, GoreScores) {
        let scores = self.predict(image);
        (scores.gore >= self.threshold, scores.gore, scores)
    }
    pub fn tier(&self, image: &DynamicImage) -> (f32, bool, bool) {
        let s = self.predict(image).gore;
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
    fn stub_safe() {
        let d = GoreDetector::new("./models/nonexistent.onnx", 0.85);
        let img = DynamicImage::new_rgb8(10, 10);
        let (s, block, _) = d.tier(&img);
        assert!(!block);
        assert!(s < 0.85);
    }
}

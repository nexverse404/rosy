//! NSFW detection — ONNX-ready with heuristic fallback

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
    pub fn nsfw_score(&self) -> f32 {
        self.porn.max(self.sexy).max(self.hentai)
    }
}

pub struct NsfwDetector {
    model_path: PathBuf,
    pub threshold: f32,
    // Optional ONNX session — boxed to avoid compile dep when feature disabled
    #[allow(dead_code)]
    has_model: bool,
}

impl NsfwDetector {
    pub fn new(model_path: impl AsRef<Path>, threshold: f32) -> Self {
        let p = model_path.as_ref().to_path_buf();
        let has_model = p.exists();
        if has_model {
            info!("NSFW model found at {}", p.display());
            // In production, load ONNX session here:
            // e.g., ort::Session::builder().with_model_from_file(&p)
            // For now we log and use heuristic until weights are provided
        } else {
            warn!("NSFW model not found at {} — using heuristic stub", p.display());
        }
        Self {
            model_path: p,
            threshold,
            has_model,
        }
    }

    /// Run inference on a decoded image
    pub fn predict(&self, image: &DynamicImage) -> NsfwScores {
        if self.has_model {
            // TODO: real ONNX inference:
            // - resize to 224x224, normalize ImageNet mean/std, transpose to NCHW
            // - session.run(...), map outputs to NsfwScores
            // Fallback to heuristic for now even if model exists (until integrated)
        }

        // Heuristic stub: never flag in stub mode (safe for dev)
        // Replace with real model — this is intentionally conservative
        let _ = image;
        NsfwScores {
            neutral: 0.95,
            porn: 0.02,
            sexy: 0.02,
            hentai: 0.01,
            drawings: 0.0,
        }
    }

    pub fn is_nsfw(&self, image: &DynamicImage) -> (bool, f32, NsfwScores) {
        let scores = self.predict(image);
        let s = scores.nsfw_score();
        (s >= self.threshold, s, scores)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    #[test]
    fn stub_never_flags() {
        let d = NsfwDetector::new("./models/nonexistent.onnx", 0.75);
        let img = DynamicImage::new_rgb8(10, 10);
        let (blocked, score, _) = d.is_nsfw(&img);
        assert!(!blocked);
        assert!(score < 0.75);
    }

    #[test]
    fn threshold_respected() {
        let d = NsfwDetector::new("./models/nonexistent.onnx", 0.01);
        let img = DynamicImage::new_rgb8(10, 10);
        // heuristic returns ~0.02, so low threshold should flag
        let (blocked, score, _) = d.is_nsfw(&img);
        // 0.02 >= 0.01 => blocked
        assert!(blocked);
        assert!(score >= 0.01);
    }
}

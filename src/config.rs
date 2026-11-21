//! Configuration loader — env + .env

use std::{collections::HashSet, env};

#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
    pub log_channel_id: Option<u64>,
    pub nsfw_threshold: f32,
    pub gore_threshold: f32,
    pub nsfw_review_threshold: f32,
    pub gore_review_threshold: f32,
    pub block_gender_images: bool,
    pub gender_threshold: f32,
    pub block_invites: bool,
    pub blocked_domains: HashSet<String>,
    pub allowed_domains: HashSet<String>,
    pub delete_message: bool,
    pub send_warning: bool,
    pub timeout_duration_seconds: u64,
    pub nsfw_model_path: String,
    pub gore_model_path: String,
    pub allow_in_nsfw_channels: bool,
    pub skip_tiny_images: bool,
    pub enable_review_queue: bool,
    // ── New: video & gender ──
    pub block_videos: bool, // recommended true until video moderation exists
    pub block_videos_recommended: bool, // flag to show warning
    pub gender_block_male: bool,
    pub gender_block_female: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let discord_token = env::var("DISCORD_TOKEN").unwrap_or_default();
        let log_channel_id = env::var("LOG_CHANNEL_ID").ok().and_then(|s| s.trim().parse::<u64>().ok()).filter(|&v| v != 0);
        let nsfw_threshold = env::var("NSFW_THRESHOLD").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.85);
        let gore_threshold = env::var("GORE_THRESHOLD").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.85);
        let nsfw_review_threshold = env::var("NSFW_REVIEW_THRESHOLD").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.75);
        let gore_review_threshold = env::var("GORE_REVIEW_THRESHOLD").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.75);
        let block_gender_images = env::var("BLOCK_GENDER_IMAGES").map(|s| s.eq_ignore_ascii_case("true") || s == "1").unwrap_or(false);
        let gender_threshold = env::var("GENDER_THRESHOLD").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.90);
        let block_invites = env::var("BLOCK_INVITES").map(|s| !(s.eq_ignore_ascii_case("false") || s == "0")).unwrap_or(true);
        let blocked_domains = env::var("BLOCKED_DOMAINS").unwrap_or_default().split(',').map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()).collect();
        let allowed_domains = env::var("ALLOWED_DOMAINS").unwrap_or_default().split(',').map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()).collect();
        let delete_message = env::var("DELETE_MESSAGE").map(|s| !(s.eq_ignore_ascii_case("false") || s == "0")).unwrap_or(true);
        let send_warning = env::var("SEND_WARNING").map(|s| !(s.eq_ignore_ascii_case("false") || s == "0")).unwrap_or(true);
        let timeout_duration_seconds = env::var("TIMEOUT_DURATION_SECONDS").or_else(|_| env::var("TIMEOUT_DURATION")).ok().and_then(|s| s.parse::<u64>().ok()).unwrap_or(300);
        let nsfw_model_path = env::var("NSFW_MODEL_PATH").unwrap_or_else(|_| "./models/nsfw_detector.onnx".into());
        let gore_model_path = env::var("GORE_MODEL_PATH").unwrap_or_else(|_| "./models/gore_detector.onnx".into());
        let allow_in_nsfw_channels = env::var("ALLOW_IN_NSFW_CHANNELS").map(|s| s.eq_ignore_ascii_case("true") || s == "1").unwrap_or(false);
        let skip_tiny_images = env::var("SKIP_TINY_IMAGES").map(|s| !(s.eq_ignore_ascii_case("false") || s == "0")).unwrap_or(true);
        let enable_review_queue = env::var("ENABLE_REVIEW_QUEUE").map(|s| !(s.eq_ignore_ascii_case("false") || s == "0")).unwrap_or(true);
        // Video blocking — recommended ON until video moderation lands
        let block_videos = env::var("BLOCK_VIDEOS").or_else(|_| env::var("BLOCK_VIDEO")).map(|s| !(s.eq_ignore_ascii_case("false") || s == "0")).unwrap_or(true);
        let block_videos_recommended = block_videos;
        let gender_block_male = env::var("GENDER_BLOCK_MALE").map(|s| s.eq_ignore_ascii_case("true") || s == "1").unwrap_or(false);
        let gender_block_female = env::var("GENDER_BLOCK_FEMALE").map(|s| s.eq_ignore_ascii_case("true") || s == "1").unwrap_or(false);

        Self {
            discord_token, log_channel_id, nsfw_threshold, gore_threshold, nsfw_review_threshold, gore_review_threshold,
            block_gender_images, gender_threshold, block_invites, blocked_domains, allowed_domains,
            delete_message, send_warning, timeout_duration_seconds, nsfw_model_path, gore_model_path,
            allow_in_nsfw_channels, skip_tiny_images, enable_review_queue,
            block_videos, block_videos_recommended, gender_block_male, gender_block_female,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.discord_token.is_empty() { return Err("DISCORD_TOKEN is not set — create .env from .env.example".into()); }
        for (name, v) in [("NSFW_THRESHOLD", self.nsfw_threshold), ("GORE_THRESHOLD", self.gore_threshold), ("NSFW_REVIEW", self.nsfw_review_threshold), ("GORE_REVIEW", self.gore_review_threshold)] {
            if !(0.0..=1.0).contains(&v) { return Err(format!("{name} must be 0.0–1.0")); }
        }
        if self.nsfw_review_threshold > self.nsfw_threshold { return Err("NSFW_REVIEW_THRESHOLD must be ≤ NSFW_THRESHOLD".into()); }
        if self.gore_review_threshold > self.gore_threshold { return Err("GORE_REVIEW_THRESHOLD must be ≤ GORE_THRESHOLD".into()); }
        Ok(())
    }

    pub fn nsfw_tier(&self, score: f32) -> Tier { if score >= self.nsfw_threshold { Tier::Block } else if score >= self.nsfw_review_threshold { Tier::Review } else { Tier::Allow } }
    pub fn gore_tier(&self, score: f32) -> Tier { if score >= self.gore_threshold { Tier::Block } else if score >= self.gore_review_threshold { Tier::Review } else { Tier::Allow } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier { Block, Review, Allow }

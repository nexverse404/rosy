//! Configuration loader — env + .env

use std::{collections::HashSet, env};

#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
    pub log_channel_id: Option<u64>,
    pub nsfw_threshold: f32,
    pub gore_threshold: f32,
    pub block_gender_images: bool,
    pub block_invites: bool,
    pub blocked_domains: HashSet<String>,
    pub allowed_domains: HashSet<String>,
    pub delete_message: bool,
    pub send_warning: bool,
    pub timeout_duration_seconds: u64,
    pub nsfw_model_path: String,
    pub gore_model_path: String,
}

impl Config {
    pub fn from_env() -> Self {
        // Load .env if present — ignore errors
        let _ = dotenvy::dotenv();

        let discord_token = env::var("DISCORD_TOKEN").unwrap_or_default();

        let log_channel_id = env::var("LOG_CHANNEL_ID")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|&v| v != 0);

        let nsfw_threshold = env::var("NSFW_THRESHOLD")
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.75);

        let gore_threshold = env::var("GORE_THRESHOLD")
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.75);

        let block_gender_images = env::var("BLOCK_GENDER_IMAGES")
            .map(|s| s.eq_ignore_ascii_case("true") || s == "1")
            .unwrap_or(false);

        let block_invites = env::var("BLOCK_INVITES")
            .map(|s| !(s.eq_ignore_ascii_case("false") || s == "0"))
            .unwrap_or(true);

        let blocked_domains = env::var("BLOCKED_DOMAINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let allowed_domains = env::var("ALLOWED_DOMAINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let delete_message = env::var("DELETE_MESSAGE")
            .map(|s| !(s.eq_ignore_ascii_case("false") || s == "0"))
            .unwrap_or(true);

        let send_warning = env::var("SEND_WARNING")
            .map(|s| !(s.eq_ignore_ascii_case("false") || s == "0"))
            .unwrap_or(true);

        let timeout_duration_seconds = env::var("TIMEOUT_DURATION_SECONDS")
            .or_else(|_| env::var("TIMEOUT_DURATION"))
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(300);

        let nsfw_model_path =
            env::var("NSFW_MODEL_PATH").unwrap_or_else(|_| "./models/nsfw_detector.onnx".into());
        let gore_model_path =
            env::var("GORE_MODEL_PATH").unwrap_or_else(|_| "./models/gore_detector.onnx".into());

        Self {
            discord_token,
            log_channel_id,
            nsfw_threshold,
            gore_threshold,
            block_gender_images,
            block_invites,
            blocked_domains,
            allowed_domains,
            delete_message,
            send_warning,
            timeout_duration_seconds,
            nsfw_model_path,
            gore_model_path,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.discord_token.is_empty() {
            return Err("DISCORD_TOKEN is not set — create .env from .env.example".into());
        }
        if !(0.0..=1.0).contains(&self.nsfw_threshold) {
            return Err("NSFW_THRESHOLD must be 0.0–1.0".into());
        }
        if !(0.0..=1.0).contains(&self.gore_threshold) {
            return Err("GORE_THRESHOLD must be 0.0–1.0".into());
        }
        Ok(())
    }
}

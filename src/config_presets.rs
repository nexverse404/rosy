//! Preset definitions for one-click admin setup

use crate::db::GuildConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Preset {
    Strict,
    Balanced,
    Permissive,
    Custom,
}

impl Preset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Preset::Strict => "Strict",
            Preset::Balanced => "Balanced",
            Preset::Permissive => "Permissive",
            Preset::Custom => "Custom",
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            Preset::Strict => "🛡️ Strict — block all NSFW/Gore/Tor/Cheap-TLD, low thresholds (0.80), timeout 10m",
            Preset::Balanced => "⚖️ Balanced — recommended, NSFW/Gore 0.85, Tor+Cheap on, review queue on (default)",
            Preset::Permissive => "🌙 Permissive — only high-confidence NSFW/Gore 0.90, Tor/Cheap off, no timeout",
            Preset::Custom => "🔧 Custom — keep your current tweaks",
        }
    }
    pub fn emoji(&self) -> &'static str {
        match self {
            Preset::Strict => "🛡️",
            Preset::Balanced => "⚖️",
            Preset::Permissive => "🌙",
            Preset::Custom => "🔧",
        }
    }
    pub fn apply_to(&self, cfg: &mut GuildConfig) {
        match self {
            Preset::Strict => {
                cfg.nsfw_threshold = Some(0.80);
                cfg.gore_threshold = Some(0.80);
                cfg.block_gender_images = Some(false);
                cfg.block_invites = Some(true);
                cfg.delete_message = Some(true);
                cfg.send_warning = Some(true);
                cfg.timeout_duration_seconds = Some(600);
                cfg.allow_in_nsfw_channels = Some(false);
            }
            Preset::Balanced => {
                cfg.nsfw_threshold = Some(0.85);
                cfg.gore_threshold = Some(0.85);
                cfg.block_gender_images = Some(false);
                cfg.block_invites = Some(true);
                cfg.delete_message = Some(true);
                cfg.send_warning = Some(true);
                cfg.timeout_duration_seconds = Some(300);
                cfg.allow_in_nsfw_channels = Some(false);
            }
            Preset::Permissive => {
                cfg.nsfw_threshold = Some(0.90);
                cfg.gore_threshold = Some(0.90);
                cfg.block_gender_images = Some(false);
                cfg.block_invites = Some(false);
                cfg.delete_message = Some(true);
                cfg.send_warning = Some(true);
                cfg.timeout_duration_seconds = Some(0);
                cfg.allow_in_nsfw_channels = Some(true);
            }
            Preset::Custom => {}
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "strict" => Some(Preset::Strict),
            "balanced" => Some(Preset::Balanced),
            "permissive" | "lenient" => Some(Preset::Permissive),
            "custom" => Some(Preset::Custom),
            _ => None,
        }
    }
}

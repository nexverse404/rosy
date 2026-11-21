//! SQLite persistence for per-guild configuration
//! Stores overrides on top of env defaults — so bot works out-of-box
//! but admins can tune per-server via /rosy slash commands.

use std::{collections::HashSet, path::Path, sync::{Arc, Mutex}};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildConfig {
    pub guild_id: u64,
    pub log_channel_id: Option<u64>,
    pub nsfw_threshold: Option<f32>,
    pub gore_threshold: Option<f32>,
    pub block_gender_images: Option<bool>,
    pub block_invites: Option<bool>,
    pub blocked_domains: HashSet<String>,
    pub allowed_domains: HashSet<String>,
    pub exempt_roles: HashSet<u64>,
    pub exempt_channels: HashSet<u64>,
    pub delete_message: Option<bool>,
    pub send_warning: Option<bool>,
    pub timeout_duration_seconds: Option<u64>,
    pub allow_in_nsfw_channels: Option<bool>,
}

impl GuildConfig {
    pub fn new(guild_id: u64) -> Self {
        Self {
            guild_id,
            log_channel_id: None,
            nsfw_threshold: None,
            gore_threshold: None,
            block_gender_images: None,
            block_invites: None,
            blocked_domains: HashSet::new(),
            allowed_domains: HashSet::new(),
            exempt_roles: HashSet::new(),
            exempt_channels: HashSet::new(),
            delete_message: None,
            send_warning: None,
            timeout_duration_seconds: None,
            allow_in_nsfw_channels: None,
        }
    }
}

#[derive(Clone)]
pub struct Db {
    inner: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(p).map_err(|e| format!("open db: {e}"))?;
        let db = Self { inner: Arc::new(Mutex::new(conn)) };
        db.migrate()?;
        info!("Opened DB at {}", p.display());
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("open mem db: {e}"))?;
        let db = Self { inner: Arc::new(Mutex::new(conn)) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.inner.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS guilds (
                guild_id INTEGER PRIMARY KEY,
                config_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS guild_domains (
                guild_id INTEGER NOT NULL,
                domain TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('block','allow')),
                PRIMARY KEY (guild_id, domain, kind)
            );
            CREATE TABLE IF NOT EXISTS guild_exemptions (
                guild_id INTEGER NOT NULL,
                target_id INTEGER NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('role','channel')),
                PRIMARY KEY (guild_id, target_id, kind)
            );
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                guild_id INTEGER NOT NULL,
                channel_id INTEGER NOT NULL,
                user_id INTEGER NOT NULL,
                action TEXT NOT NULL,
                reason TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .map_err(|e| format!("migrate: {e}"))?;
        Ok(())
    }

    pub fn get_guild(&self, guild_id: u64) -> Result<GuildConfig, String> {
        let conn = self.inner.lock().unwrap();
        let json: Option<String> = conn
            .query_row(
                "SELECT config_json FROM guilds WHERE guild_id = ?1",
                [guild_id as i64],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("query guild: {e}"))?;

        if let Some(j) = json {
            serde_json::from_str(&j).map_err(|e| format!("decode guild: {e}"))
        } else {
            Ok(GuildConfig::new(guild_id))
        }
    }

    pub fn upsert_guild(&self, cfg: &GuildConfig) -> Result<(), String> {
        let conn = self.inner.lock().unwrap();
        let json = serde_json::to_string(cfg).map_err(|e| format!("encode: {e}"))?;
        conn.execute(
            "INSERT INTO guilds (guild_id, config_json) VALUES (?1, ?2)
             ON CONFLICT(guild_id) DO UPDATE SET config_json = excluded.config_json",
            params![cfg.guild_id as i64, json],
        )
        .map_err(|e| format!("upsert: {e}"))?;
        Ok(())
    }

    pub fn log_action(&self, guild_id: u64, channel_id: u64, user_id: u64, action: &str, reason: &str) {
        let conn = match self.inner.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now = chrono::Utc::now().to_rfc3339();
        if let Err(e) = conn.execute(
            "INSERT INTO audit_log (guild_id, channel_id, user_id, action, reason, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
            params![guild_id as i64, channel_id as i64, user_id as i64, action, reason, now],
        ) {
            warn!("audit log failed: {e}");
        }
    }

    // Domain helpers for slash commands
    pub fn add_blocked_domain(&self, guild_id: u64, domain: &str) -> Result<(), String> {
        let mut cfg = self.get_guild(guild_id)?;
        cfg.blocked_domains.insert(domain.to_ascii_lowercase());
        self.upsert_guild(&cfg)
    }

    pub fn remove_blocked_domain(&self, guild_id: u64, domain: &str) -> Result<(), String> {
        let mut cfg = self.get_guild(guild_id)?;
        cfg.blocked_domains.remove(&domain.to_ascii_lowercase());
        self.upsert_guild(&cfg)
    }

    pub fn add_allowed_domain(&self, guild_id: u64, domain: &str) -> Result<(), String> {
        let mut cfg = self.get_guild(guild_id)?;
        cfg.allowed_domains.insert(domain.to_ascii_lowercase());
        self.upsert_guild(&cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let db = Db::open_in_memory().unwrap();
        let mut cfg = GuildConfig::new(123);
        cfg.nsfw_threshold = Some(0.8);
        cfg.blocked_domains.insert("evil.com".into());
        db.upsert_guild(&cfg).unwrap();
        let got = db.get_guild(123).unwrap();
        assert_eq!(got.nsfw_threshold, Some(0.8));
        assert!(got.blocked_domains.contains("evil.com"));
    }
}

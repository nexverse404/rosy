//! Rosy Guard — Discord moderation bot (Rust + Serenity)
//! Easy config management for admins via dashboard + presets.
//! Scaled for 100+ guilds: global commands, WAL DB, non-blocking scans, sharded-ready.

mod commands;
mod config;
mod config_presets;
mod dashboard;
mod db;
mod embeds;
mod filterlist;
mod handlers;
mod interactions;
mod moderation;
mod permissions;
mod utils;

use std::sync::Arc;
use serenity::{all::{GuildId, Interaction, Message, MessageUpdateEvent, Ready}, async_trait, prelude::*};
use tracing::{info, warn};

use config::Config;
use db::Db;
use filterlist::FilterListManager;
use moderation::{image_filter::ImageFilter, link_scanner::LinkScanner};

struct Handler {
    config: Config,
    db: Db,
    filterlists: FilterListManager,
    image_filter: Arc<ImageFilter>,
    link_scanner: Arc<LinkScanner>,
    http_client: reqwest::Client,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // 100+ guilds: don't block gateway — spawn scan so one slow image fetch (15s) never stalls other guilds
        let ctx2 = ctx.clone();
        let (cfg, db, img, scanner, http) = (self.config.clone(), self.db.clone(), Arc::clone(&self.image_filter), Arc::clone(&self.link_scanner), self.http_client.clone());
        tokio::spawn(async move {
            let _ = handlers::handle_message(&ctx2, &msg, &cfg, &db, &img, &scanner, &http).await;
        });
    }
    async fn message_update(&self, ctx: Context, _old: Option<Message>, _new: Option<Message>, event: MessageUpdateEvent) {
        let Some(guild_id) = event.guild_id else { return; };
        let msg = match ctx.http.get_message(event.channel_id, event.id).await {
            Ok(m) => m,
            Err(e) => { warn!("Fetch edited {} failed: {:?}", event.id, e); return; }
        };
        let ctx2 = ctx.clone();
        let (cfg, db, img, scanner, http) = (self.config.clone(), self.db.clone(), Arc::clone(&self.image_filter), Arc::clone(&self.link_scanner), self.http_client.clone());
        tokio::spawn(async move {
            let _ = handlers::handle_message(&ctx2, &msg, &cfg, &db, &img, &scanner, &http).await;
            info!("Re-scanned edited {} in guild {}", event.id, guild_id);
        });
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(cmd) => commands::handle_interaction(&ctx, &cmd, &self.db).await,
            Interaction::Component(comp) => interactions::handle_component(&ctx, &comp, &self.db).await,
            _ => {}
        }
    }
    async fn guild_create(&self, ctx: Context, guild: serenity::all::Guild, is_new: Option<bool>) {
        // 100+ guilds: avoid per-guild create_command (rate-limits at ~200/day/guild). Global commands registered once in `ready`.
        // Only ensure DB row exists and log; global commands propagate to new guilds within ~1h (Discord) — instant enough at scale.
        let guild_id = guild.id;
        let _ = self.db.get_guild(guild_id.get());
        let fl = self.filterlists.get();
        info!("Guild {} ({} members) — filterlists NSFW {} | Gore {} | Tor {} | Cheap-TLD {} | thr NSFW {:.2}/{:.2} Gore {:.2}/{:.2} | video recommended ON | DB guilds={}",
            guild.name, guild.member_count, fl.nsfw_count, fl.gore_count, fl.tor_count, fl.cheap_tlds_count,
            self.config.nsfw_threshold, self.config.nsfw_review_threshold, self.config.gore_threshold, self.config.gore_review_threshold,
            self.db.guild_count());
        if is_new.unwrap_or(false) {
            if let Some(sys) = guild.system_channel_id {
                let embed = embeds::info_embed(
                    "🌹 Rosy Guard installed!",
                    &format!("**Welcome!** Rosy blocks NSFW/Gore/Tor/Cheap-Sus + images + video (recommended).\n\n\
                        **Get started in 30s:**\n\
                        • `/rosy setup` — step-by-step wizard\n\
                        • `/rosy dashboard` — click buttons to config (recommended for admins)\n\
                        • `/rosy preset balanced` — one-click apply Balanced preset (0.85, video ON)\n\
                        • `/rosy gender channel:#nsfw action:block gender:female` — per-channel gender\n\
                        • `/rosy video enable:true` — video blocking (recommended ON, not moderatable yet)\n\n\
                        *Filterlists: NSFW {} | Gore {} | Tor {} | Cheap-TLD {} — auto-updated from GitHub.*\n\
                        *All per-server, instant, no restart. Scales to 100+ servers.*", fl.nsfw_count, fl.gore_count, fl.tor_count, fl.cheap_tlds_count)
                );
                let _ = sys.send_message(&ctx.http, serenity::all::CreateMessage::new().embed(embed)).await;
            }
        }
    }
    async fn ready(&self, ctx: Context, ready: Ready) {
        let fl = self.filterlists.get();
        info!("{} connected — {} guilds — NSFW:{} Gore:{} Tor:{} Cheap-TLD:{} — block ≥0.85 review 0.75–0.85 — video recommended ON — easy config dashboard enabled — 100+ guild fleet ready (guild_id shard hint, WAL DB)",
            ready.user.name, ready.guilds.len(), fl.nsfw_count, fl.gore_count, fl.tor_count, fl.cheap_tlds_count);
        // Global commands only — one call scales to 100+ guilds (vs N per-guild calls that rate-limit)
        for cmd in commands::register_commands() { if let Err(e) = serenity::all::Command::create_global_command(&ctx.http, cmd).await { warn!("Global cmd failed: {:?}", e); } }
        let mgr = self.filterlists.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400));
            loop { interval.tick().await; if let Err(e) = mgr.refresh_once().await { warn!("Filterlist refresh failed: {}", e); } else { info!("Filterlists refreshed from GitHub"); } }
        });
        let _ = ctx;
    }
    async fn cache_ready(&self, _ctx: Context, guilds: Vec<GuildId>) { info!("Cache ready — {} guilds (100+ fleet: single shard handles ~100–1000 guilds; auto-shard at ~2000)", guilds.len()); }
}

#[tokio::main]
async fn main() {
    utils::logger::init();
    let config = Config::from_env();
    if let Err(e) = config.validate() { error!("Invalid config: {}", e); std::process::exit(1); }
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/rosy.db".into());
    let db = Db::open(&db_path).unwrap_or_else(|e| { error!("DB open {} failed: {} → in-memory", db_path, e); Db::open_in_memory().expect("mem db") });
    let filterlists_mgr = FilterListManager::new();
    let fl = filterlists_mgr.get();
    info!(nsfw_block=config.nsfw_threshold, nsfw_review=config.nsfw_review_threshold, gore_block=config.gore_threshold, gore_review=config.gore_review_threshold, nsfw_count=fl.nsfw_count, gore_count=fl.gore_count, tor_count=fl.tor_count, cheap_tlds_count=fl.cheap_tlds_count, nsfw_src=%fl.nsfw_source, gore_src=%fl.gore_source, tor_src=%fl.tor_source, tld_src=%fl.cheap_tlds_source, video_block=config.block_videos, db=%db_path, guilds=%db.guild_count(), "Starting Rosy Guard 🌹 — 100+ guild ready: easy config dashboard + per-channel gender/video (video recommended ON), WAL DB, global commands, spawned scans");
    let nsfw = NsfwDetector::new_with_review(&config.nsfw_model_path, config.nsfw_threshold, config.nsfw_review_threshold);
    let gore = GoreDetector::new_with_review(&config.gore_model_path, config.gore_threshold, config.gore_review_threshold);
    let gender_enabled = config.block_gender_images || config.gender_block_male || config.gender_block_female || true;
    let image_filter = Arc::new(ImageFilter::new(nsfw, gore, gender_enabled).with_gender_threshold(config.gender_threshold).with_skip_tiny(config.skip_tiny_images));
    let link_scanner = Arc::new(LinkScanner::new(fl.clone(), config.allowed_domains.clone(), config.block_invites));
    let http_client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).user_agent("rosy-guard/0.1 (+https://github.com/nexverse404/rosy) — filterlists: blocklistproject/Lists + aegis-blocklist + hagezi — 100+ guilds").build().expect("http client");
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILDS | GatewayIntents::GUILD_MEMBERS;
    let handler = Handler { config: config.clone(), db, filterlists: filterlists_mgr, image_filter, link_scanner, http_client };
    // Sharding: serenity auto-shards when `num_shards` is set or via env SHARD_COUNT. For 100 guilds 1 shard is fine (~1000/guilds per shard).
    // At ~2000+ guilds set SHARDS env and use Client::builder(token, intents).num_shards(N) or serenity::client::ClientBuilder shard helpers.
    let mut client = Client::builder(&config.discord_token, intents).event_handler(handler).await.expect("client");
    if let Err(why) = client.start().await { error!("Client error: {:?}", why); }
}

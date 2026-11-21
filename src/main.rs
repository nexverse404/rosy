//! Rosy Guard — Discord moderation bot (Rust + Serenity)
//! Blocks gender-specific images, NSFW and gore photos and links — with false-positive safety.

mod commands;
mod config;
mod db;
mod handlers;
mod moderation;
mod permissions;
mod utils;

use std::sync::Arc;
use serenity::{all::{GuildId, Interaction, Message, MessageUpdateEvent, Ready}, async_trait, prelude::*};
use tracing::{error, info, warn};

use config::Config;
use db::Db;
use moderation::{gore_detector::GoreDetector, image_filter::ImageFilter, link_scanner::LinkScanner, nsfw_detector::NsfwDetector};

struct Handler {
    config: Config,
    db: Db,
    image_filter: Arc<ImageFilter>,
    link_scanner: Arc<LinkScanner>,
    http_client: reqwest::Client,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let _ = handlers::handle_message(&ctx, &msg, &self.config, &self.db, &self.image_filter, &self.link_scanner, &self.http_client).await;
    }
    async fn message_update(&self, ctx: Context, _old: Option<Message>, _new: Option<Message>, event: MessageUpdateEvent) {
        let Some(guild_id) = event.guild_id else { return; };
        let msg = match ctx.http.get_message(event.channel_id, event.id).await {
            Ok(m) => m,
            Err(e) => { warn!("Fetch edited {} failed: {:?}", event.id, e); return; }
        };
        let _ = handlers::handle_message(&ctx, &msg, &self.config, &self.db, &self.image_filter, &self.link_scanner, &self.http_client).await;
        info!("Re-scanned edited {} in guild {}", event.id, guild_id);
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(cmd) = interaction { commands::handle_interaction(&ctx, &cmd, &self.db).await; }
    }
    async fn guild_create(&self, ctx: Context, guild: serenity::all::Guild, _is_new: Option<bool>) {
        let guild_id = guild.id;
        for cmd in commands::register_commands() {
            if let Err(e) = guild_id.create_command(&ctx.http, cmd).await { warn!("Register cmd for {} failed: {:?}", guild_id, e); }
        }
        let _ = self.db.get_guild(guild_id.get());
        info!("Guild {} ({} members) — thresholds NSFW {:.2}/review {:.2} | Gore {:.2}/review {:.2}", guild.name, guild.member_count, self.config.nsfw_threshold, self.config.nsfw_review_threshold, self.config.gore_threshold, self.config.gore_review_threshold);
    }
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} connected — {} guilds — false-positive safe: block ≥0.85, review 0.75–0.85 → mod queue", ready.user.name, ready.guilds.len());
        for cmd in commands::register_commands() { if let Err(e) = serenity::all::Command::create_global_command(&ctx.http, cmd).await { warn!("Global cmd failed: {:?}", e); } }
        let _ = ctx;
    }
    async fn cache_ready(&self, _ctx: Context, guilds: Vec<GuildId>) { info!("Cache ready — {} guilds", guilds.len()); }
}

#[tokio::main]
async fn main() {
    utils::logger::init();
    let config = Config::from_env();
    if let Err(e) = config.validate() { error!("Invalid config: {}", e); std::process::exit(1); }
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/rosy.db".into());
    let db = Db::open(&db_path).unwrap_or_else(|e| { error!("DB open {} failed: {} → in-memory", db_path, e); Db::open_in_memory().expect("mem db") });
    info!(nsfw_block=config.nsfw_threshold, nsfw_review=config.nsfw_review_threshold, gore_block=config.gore_threshold, gore_review=config.gore_review_threshold, gender_thr=config.gender_threshold, skip_tiny=config.skip_tiny_images, review_queue=config.enable_review_queue, db=%db_path, "Starting Rosy Guard 🌹 — false-positive safe");
    let nsfw = NsfwDetector::new_with_review(&config.nsfw_model_path, config.nsfw_threshold, config.nsfw_review_threshold);
    let gore = GoreDetector::new_with_review(&config.gore_model_path, config.gore_threshold, config.gore_review_threshold);
    let image_filter = Arc::new(ImageFilter::new(nsfw, gore, config.block_gender_images).with_gender_threshold(config.gender_threshold).with_skip_tiny(config.skip_tiny_images));
    let link_scanner = Arc::new(LinkScanner::new(config.blocked_domains.clone(), config.allowed_domains.clone(), config.block_invites));
    let http_client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).user_agent("rosy-guard/0.1 (+https://github.com/nexverse404/rosy) — false-positive safe").build().expect("http client");
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILDS | GatewayIntents::GUILD_MEMBERS;
    let handler = Handler { config: config.clone(), db, image_filter, link_scanner, http_client };
    let mut client = Client::builder(&config.discord_token, intents).event_handler(handler).await.expect("client");
    if let Err(why) = client.start().await { error!("Client error: {:?}", why); }
}

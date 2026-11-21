//! Rosy Guard — Discord moderation bot (Rust + Serenity)
//! Blocks gender-specific images, NSFW and gore photos and links.

mod commands;
mod config;
mod db;
mod handlers;
mod moderation;
mod permissions;
mod utils;

use std::sync::Arc;

use serenity::{
    all::{GuildId, Interaction, Message, MessageUpdateEvent, Ready},
    async_trait,
    prelude::*,
};
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
        // Edited messages can add new URLs/images → re-scan
        // We need to fetch the full message
        let Some(guild_id) = event.guild_id else { return; };
        let channel_id = event.channel_id;
        let msg_id = event.id;
        
        // Fetch via HTTP
        let msg = match ctx.http.get_message(channel_id, msg_id).await {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to fetch edited message {}: {:?}", msg_id, e);
                return;
            }
        };
        // Reuse same handler (will check exemptions again)
        let _ = handlers::handle_message(&ctx, &msg, &self.config, &self.db, &self.image_filter, &self.link_scanner, &self.http_client).await;
        info!("Re-scanned edited message {} in guild {}", msg_id, guild_id);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(cmd) = interaction {
            commands::handle_interaction(&ctx, &cmd, &self.db).await;
        }
    }

    async fn guild_create(&self, ctx: Context, guild: serenity::all::Guild, _is_new: Option<bool>) {
        // Register slash commands per guild for instant availability (global takes 1h)
        let guild_id = guild.id;
        let cmds = commands::register_commands();
        for cmd in cmds {
            if let Err(e) = guild_id.create_command(&ctx.http, cmd).await {
                warn!("Failed to register command for guild {}: {:?}", guild_id, e);
            }
        }
        // Ensure DB entry exists
        let _ = self.db.get_guild(guild_id.get());
        info!("Guild available: {} ({} members)", guild.name, guild.member_count);
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected — serving {} guilds", ready.user.name, ready.guilds.len());
        
        // Global fallback registration (propagates in ~1 hour)
        for cmd in commands::register_commands() {
            if let Err(e) = serenity::all::Command::create_global_command(&ctx.http, cmd).await {
                warn!("Global command register failed: {:?}", e);
            }
        }
        let _ = ctx;
    }

    async fn cache_ready(&self, _ctx: Context, guilds: Vec<GuildId>) {
        info!("Cache ready — {} guilds", guilds.len());
    }
}

#[tokio::main]
async fn main() {
    utils::logger::init();

    let config = Config::from_env();
    if let Err(e) = config.validate() {
        error!("Invalid config: {}", e);
        std::process::exit(1);
    }

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/rosy.db".into());
    let db = Db::open(&db_path).unwrap_or_else(|e| {
        error!("DB open failed ({}): falling back to in-memory", e);
        Db::open_in_memory().expect("mem db")
    });

    info!(
        nsfw_threshold = config.nsfw_threshold,
        gore_threshold = config.gore_threshold,
        block_gender = config.block_gender_images,
        db = %db_path,
        "Starting Rosy Guard 🌹"
    );

    // Build detectors (global defaults — per-guild overrides applied at scan time)
    let nsfw = NsfwDetector::new(&config.nsfw_model_path, config.nsfw_threshold);
    let gore = GoreDetector::new(&config.gore_model_path, config.gore_threshold);
    let image_filter = Arc::new(ImageFilter::new(nsfw, gore, config.block_gender_images));

    let link_scanner = Arc::new(LinkScanner::new(
        config.blocked_domains.clone(),
        config.allowed_domains.clone(),
        config.block_invites,
    ));

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("rosy-guard/0.1 (+https://github.com/nexverse404/rosy)")
        .build()
        .expect("http client");

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_MESSAGE_TYPING; // for cache completeness

    let handler = Handler {
        config: config.clone(),
        db,
        image_filter,
        link_scanner,
        http_client,
    };

    let mut client = Client::builder(&config.discord_token, intents)
        .event_handler(handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}

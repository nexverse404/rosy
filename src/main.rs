//! Rosy Guard — Discord moderation bot (Rust + Serenity)
//! Blocks gender-specific images, NSFW and gore photos and links.

mod config;
mod moderation;
mod utils;

use std::{collections::HashSet, sync::Arc};

use serenity::{
    all::{ChannelId, CreateEmbed, CreateEmbedFooter, CreateMessage, GuildId, Message, Ready},
    async_trait,
    prelude::*,
};
use tracing::{error, info, warn};

use config::Config;
use moderation::{image_filter::ImageFilter, link_scanner::LinkScanner};
use moderation::{gore_detector::GoreDetector, nsfw_detector::NsfwDetector};

struct Handler {
    config: Config,
    image_filter: Arc<ImageFilter>,
    link_scanner: Arc<LinkScanner>,
    http_client: reqwest::Client,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore bots and webhooks
        if msg.author.bot {
            return;
        }

        // Allow exempt roles/channels later — stub: check if needed
        let guild_id = match msg.guild_id {
            Some(g) => g,
            None => return, // DM — ignore
        };

        // Collect all URLs to scan: content + attachments + embeds
        let mut image_urls: Vec<String> = Vec::new();
        let mut text_to_scan = msg.content.clone();

        // Attachments
        for att in &msg.attachments {
            // Discord CDN URL
            let url = att.url.clone();
            // Heuristic: treat image/video attachments as image URLs if content type is image
            if let Some(ct) = &att.content_type {
                if ct.starts_with("image/") {
                    image_urls.push(url.clone());
                }
            } else if is_likely_image_url(&url) {
                image_urls.push(url.clone());
            }
            // Also add URL to text scan for link checking
            text_to_scan.push(' ');
            text_to_scan.push_str(&url);
        }

        // Embeds often contain image/thumbnail URLs
        for embed in &msg.embeds {
            if let Some(url) = &embed.url {
                text_to_scan.push(' ');
                text_to_scan.push_str(url);
            }
            if let Some(img) = &embed.image {
                image_urls.push(img.url.clone());
            }
            if let Some(thumb) = &embed.thumbnail {
                image_urls.push(thumb.url.clone());
            }
        }

        // Sticker URLs (if any)
        for sticker in &msg.sticker_items {
            // Stickers don't have direct URL in this model, but can be fetched via sticker id
            // For now, we log and skip detailed scan
            warn!("Sticker present {} — scan not yet implemented", sticker.id);
        }

        // Also extract inline URLs from content for image scanning (e.g. direct image links)
        let inline_urls = self.link_scanner.extract_urls(&msg.content);
        for u in inline_urls.iter() {
            if is_likely_image_url(u) {
                image_urls.push(u.clone());
            }
        }
        // Deduplicate
        let mut seen = HashSet::new();
        image_urls.retain(|u| seen.insert(u.clone()));

        // --- Link scanning ---
        let link_results = self.link_scanner.scan_text(&text_to_scan);
        let blocked_links: Vec<_> = link_results.iter().filter(|r| r.blocked).collect();

        // --- Image scanning ---
        let mut image_results = Vec::new();
        for url in &image_urls {
            let res = self.image_filter.scan_url(url, &self.http_client).await;
            if res.blocked {
                image_results.push(res);
            } else if let Some(err) = res.error {
                warn!("Image scan error for {}: {:?}", url, err);
            }
        }

        // Decide action
        let mut violations: Vec<String> = Vec::new();
        for bl in &blocked_links {
            violations.push(format!("🔗 Blocked link `{}` — {}", bl.url, bl.reason));
        }
        for ir in &image_results {
            let reasons = ir.reasons.join(", ");
            violations.push(format!("🖼️ Blocked image <{}> — {}", ir.url, reasons));
        }

        if violations.is_empty() {
            return;
        }

        info!(
            guild = %guild_id,
            channel = %msg.channel_id,
            author = %msg.author.tag(),
            "Violation detected: {:?}",
            violations
        );

        // Build moderation log embed
        let description = violations.join("\n");
        let footer_text = format!("User: {} ({})", msg.author.tag(), msg.author.id);
        let embed = CreateEmbed::new()
            .title("Rosy Guard — violation blocked")
            .description(&description)
            .footer(CreateEmbedFooter::new(footer_text))
            .field("Channel", format!("<#{}>", msg.channel_id), true)
            .field("Message ID", msg.id.to_string(), true)
            .color(0xFF2D55);

        // Log to mod channel if configured
        if let Some(log_channel) = self.config.log_channel_id {
            let log_id = ChannelId::new(log_channel);
            let content = format!(
                "**Violation** by {} in <#{}> — {}",
                msg.author.mention(),
                msg.channel_id,
                description
            );
            if let Err(e) = log_id
                .send_message(&ctx.http, CreateMessage::new().content(content).embed(embed))
                .await
            {
                error!("Failed to send log message: {:?}", e);
            }
        }

        // Delete offending message
        if self.config.delete_message {
            if let Err(e) = msg.delete(&ctx.http).await {
                error!("Failed to delete message {}: {:?}", msg.id, e);
            } else {
                info!("Deleted violating message {}", msg.id);
            }
        }

        // Warn user
        if self.config.send_warning {
            let warning = format!(
                "{}, your message was removed for violating server safety rules:\n{}",
                msg.author.mention(),
                description
            );
            // Try to send in channel (ephemeral-like, auto-delete after timeout not supported without interaction)
            if let Err(e) = msg
                .channel_id
                .send_message(&ctx.http, CreateMessage::new().content(warning))
                .await
            {
                warn!("Failed to send warning: {:?}", e);
            }
        }

        // Timeout user
        if self.config.timeout_duration_seconds > 0 {
            let until = chrono::Utc::now()
                + chrono::Duration::seconds(self.config.timeout_duration_seconds as i64);
            if let Err(e) = guild_id
                .edit_member(
                    &ctx.http,
                    msg.author.id,
                    serenity::builder::EditMember::new().disable_communication_until_datetime(&until.into()),
                )
                .await
            {
                warn!("Failed to timeout {}: {:?}", msg.author.tag(), e);
            } else {
                info!(
                    "Timed out {} until {}",
                    msg.author.tag(),
                    until.format("%Y-%m-%d %H:%M:%S UTC")
                );
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected — serving {} guilds", ready.user.name, ready.guilds.len());
    }
}

fn is_likely_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
        || lower.contains("cdn.discordapp.com/attachments")
        || lower.contains("media.discordapp.net")
}

#[tokio::main]
async fn main() {
    utils::logger::init();

    let config = Config::from_env();
    if let Err(e) = config.validate() {
        error!("Invalid config: {}", e);
        std::process::exit(1);
    }

    info!(
        nsfw_threshold = config.nsfw_threshold,
        gore_threshold = config.gore_threshold,
        block_gender = config.block_gender_images,
        "Starting Rosy Guard"
    );

    // Build detectors
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
        | GatewayIntents::GUILD_MEMBERS;

    let handler = Handler {
        config: config.clone(),
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

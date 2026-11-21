//! Message handling extracted for testability

use std::{collections::HashSet, sync::Arc};
use serenity::all::{ChannelId, CreateEmbed, CreateEmbedFooter, CreateMessage, GuildId, Message};
use serenity::prelude::Context;
use tracing::{error, info, warn};

use crate::{config::Config, db::Db, moderation::{image_filter::ImageFilter, link_scanner::LinkScanner}, permissions};

pub struct ScanOutcome {
    pub violations: Vec<String>,
    pub image_results: Vec<crate::moderation::image_filter::ImageScanResult>,
    pub link_results: Vec<crate::moderation::link_scanner::LinkResult>,
}

pub fn is_likely_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
        || lower.ends_with(".avif")
        || lower.contains("cdn.discordapp.com/attachments")
        || lower.contains("media.discordapp.net")
        || lower.contains("cdn.discordapp.com/embed")
}

pub async fn handle_message(
    ctx: &Context,
    msg: &Message,
    global_config: &Config,
    db: &Db,
    image_filter: &Arc<ImageFilter>,
    link_scanner: &Arc<LinkScanner>,
    http_client: &reqwest::Client,
) -> Option<ScanOutcome> {
    if msg.author.bot {
        return None;
    }
    let guild_id = msg.guild_id?;
    
    // Resolve guild-specific config
    let guild_cfg = db.get_guild(guild_id.get()).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id.get()));
    
    // Determine effective thresholds / behaviour (guild overrides env)
    let nsfw_threshold = guild_cfg.nsfw_threshold.unwrap_or(global_config.nsfw_threshold);
    let gore_threshold = guild_cfg.gore_threshold.unwrap_or(global_config.gore_threshold);
    let block_gender = guild_cfg.block_gender_images.unwrap_or(global_config.block_gender_images);
    let block_invites = guild_cfg.block_invites.unwrap_or(global_config.block_invites);
    let log_channel = guild_cfg.log_channel_id.or(global_config.log_channel_id);
    let delete = guild_cfg.delete_message.unwrap_or(global_config.delete_message);
    let warn_user = guild_cfg.send_warning.unwrap_or(global_config.send_warning);
    let timeout_secs = guild_cfg.timeout_duration_seconds.unwrap_or(global_config.timeout_duration_seconds);

    // Check if channel is NSFW (need cache)
    let is_nsfw_channel = ctx.cache.guild_channel(msg.channel_id).map(|c| c.nsfw).unwrap_or(false);

    // Exempt check (roles + channel + NSFW bypass)
    if let Some(member) = &msg.member {
        if let Some(reason) = permissions::is_exempt(
            &member.roles,
            msg.channel_id.get(),
            &guild_cfg,
            is_nsfw_channel,
        ) {
            info!("Exempt message from {}: {}", msg.author.tag(), reason);
            return None;
        }
    } else if guild_cfg.exempt_channels.contains(&msg.channel_id.get()) {
        info!("Exempt channel {} for message {}", msg.channel_id, msg.id);
        return None;
    }
    if guild_cfg.allow_in_nsfw_channels.unwrap_or(false) && is_nsfw_channel {
        return None;
    }

    // Build effective link scanner per-guild (merge global + guild)
    let mut blocked = global_config.blocked_domains.clone();
    blocked.extend(guild_cfg.blocked_domains.clone());
    let mut allowed = global_config.allowed_domains.clone();
    allowed.extend(guild_cfg.allowed_domains.clone());

    // We use the passed link_scanner for URL extraction but re-evaluate block status with merged sets
    // For now, create a temporary scanner with merged domains
    let effective_scanner = LinkScanner::new(blocked.clone(), allowed.clone(), block_invites);

    // Collect image URLs
    let mut image_urls: Vec<String> = Vec::new();
    let mut text_to_scan = msg.content.clone();

    for att in &msg.attachments {
        let url = att.url.clone();
        if let Some(ct) = &att.content_type {
            if ct.starts_with("image/") {
                image_urls.push(url.clone());
            }
        } else if is_likely_image_url(&url) {
            image_urls.push(url.clone());
        }
        text_to_scan.push(' ');
        text_to_scan.push_str(&url);
    }
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
    // Sticker handling: log
    if !msg.sticker_items.is_empty() {
        for s in &msg.sticker_items {
            warn!("Sticker {} in {} — TODO scan via sticker CDN", s.id, msg.id);
        }
    }
    // Inline image links
    for u in link_scanner.extract_urls(&msg.content) {
        if is_likely_image_url(&u) {
            image_urls.push(u);
        }
    }
    let mut seen = HashSet::new();
    image_urls.retain(|u| seen.insert(u.clone()));

    // Link scan with effective scanner
    let link_results = effective_scanner.scan_text(&text_to_scan);
    let blocked_links: Vec<_> = link_results.iter().filter(|r| r.blocked).cloned().collect();

    // Image scan — we need to respect per-guild thresholds, but ImageFilter is global
    // For MVP we reuse global thresholds; TODO: make ImageFilter per-guild or pass thresholds
    // We work around by checking scores against effective thresholds after scan
    let mut image_results = Vec::new();
    for url in &image_urls {
        let mut res = image_filter.scan_url(url, http_client).await;
        // Re-evaluate against effective thresholds (since detectors use global threshold)
        // Our detectors return blocked based on their own threshold, but we have scores
        // So we recompute: if score >= effective threshold then blocked
        let nsfw_blocked = res.nsfw_score >= nsfw_threshold;
        let gore_blocked = res.gore_score >= gore_threshold;
        // If global detector said not blocked but effective says blocked, we need to flag
        // However our ImageScanResult already contains reasons based on global threshold
        // So we reconstruct violations here
        if nsfw_blocked || gore_blocked || res.blocked {
            if nsfw_blocked && !res.reasons.iter().any(|r| r.contains("NSFW")) {
                res.reasons.push(format!("NSFW ({:.2} ≥ {:.2})", res.nsfw_score, nsfw_threshold));
            }
            if gore_blocked && !res.reasons.iter().any(|r| r.contains("Gore")) {
                res.reasons.push(format!("Gore ({:.2} ≥ {:.2})", res.gore_score, gore_threshold));
            }
            res.blocked = nsfw_blocked || gore_blocked || res.blocked;
        } else {
            // Not blocked under effective thresholds — skip
            if res.error.is_some() {
                warn!("Image scan error for {}: {:?}", url, res.error);
            }
            continue;
        }
        if res.blocked {
            image_results.push(res);
        } else if let Some(err) = res.error {
            warn!("Image scan error for {}: {}", url, err);
        }
    }

    let mut violations: Vec<String> = Vec::new();
    for bl in &blocked_links {
        violations.push(format!("🔗 Blocked link `{}` — {}", bl.url, bl.reason));
    }
    for ir in &image_results {
        let reasons = ir.reasons.join(", ");
        violations.push(format!("🖼️ Blocked image <{}> — {}", ir.url, reasons));
    }

    if violations.is_empty() {
        return None;
    }

    info!(
        guild = %guild_id,
        channel = %msg.channel_id,
        author = %msg.author.tag(),
        "Violation: {:?}",
        violations
    );

    // Mod log embed
    let description = violations.join("\n");
    let footer_text = format!("User: {} ({})", msg.author.tag(), msg.author.id);
    let embed = CreateEmbed::new()
        .title("Rosy Guard — violation blocked")
        .description(&description)
        .footer(CreateEmbedFooter::new(footer_text))
        .field("Channel", format!("<#{}>", msg.channel_id), true)
        .field("Message ID", msg.id.to_string(), true)
        .color(0xFF2D55);

    if let Some(log_channel_id) = log_channel {
        let log_id = ChannelId::new(log_channel_id);
        let content = format!(
            "**Violation** by {} in <#{}> — {}\n**Scores**: NSFW thr {:.2} | Gore thr {:.2} | Gender filter {}",
            msg.author.mention(),
            msg.channel_id,
            description,
            nsfw_threshold,
            gore_threshold,
            if block_gender { "on" } else { "off" }
        );
        if let Err(e) = log_id
            .send_message(&ctx.http, CreateMessage::new().content(content).embed(embed))
            .await
        {
            error!("Failed to send log: {:?}", e);
        }
    }

    if delete {
        if let Err(e) = msg.delete(&ctx.http).await {
            error!("Failed to delete {}: {:?}", msg.id, e);
        } else {
            info!("Deleted {}", msg.id);
            db.log_action(guild_id.get(), msg.channel_id.get(), msg.author.id.get(), "delete", &violations.join("; "));
        }
    }

    if warn_user {
        let warning = format!(
            "{}, your message was removed for violating **{}** safety rules:\n{}\n\
            If this was a mistake, contact a moderator.",
            msg.author.mention(),
            ctx.cache.current_user().name,
            description
        );
        if let Err(e) = msg.channel_id.send_message(&ctx.http, CreateMessage::new().content(warning)).await {
            warn!("Warn failed: {:?}", e);
        }
    }

    if timeout_secs > 0 {
        let until = chrono::Utc::now() + chrono::Duration::seconds(timeout_secs as i64);
        if let Err(e) = guild_id
            .edit_member(
                &ctx.http,
                msg.author.id,
                serenity::builder::EditMember::new().disable_communication_until_datetime(&until.into()),
            )
            .await
        {
            warn!("Timeout {} failed: {:?}", msg.author.tag(), e);
        } else {
            info!("Timed out {} until {}", msg.author.tag(), until.format("%Y-%m-%d %H:%M:%S UTC"));
            db.log_action(guild_id.get(), msg.channel_id.get(), msg.author.id.get(), "timeout", &format!("{}s: {}", timeout_secs, violations.join("; ")));
        }
    }

    Some(ScanOutcome {
        violations,
        image_results,
        link_results: blocked_links,
    })
}

//! Message handling — with tiered false-positive safety

use std::{collections::HashSet, sync::Arc};
use serenity::all::{ChannelId, CreateEmbed, CreateEmbedFooter, CreateMessage};
use serenity::prelude::Context;
use serenity::all::Message;
use tracing::{error, info, warn};

use crate::{config::{Config, Tier}, db::Db, moderation::{image_filter::ImageFilter, link_scanner::LinkScanner}, permissions};

pub struct ScanOutcome {
    pub violations: Vec<String>,
    pub reviews: Vec<String>,
    pub image_results: Vec<crate::moderation::image_filter::ImageScanResult>,
    pub link_results: Vec<crate::moderation::link_scanner::LinkResult>,
}

pub fn is_likely_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".gif") || lower.ends_with(".webp") || lower.ends_with(".bmp") || lower.ends_with(".avif")
        || lower.contains("cdn.discordapp.com/attachments") || lower.contains("media.discordapp.net") || lower.contains("cdn.discordapp.com/embed")
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
    if msg.author.bot { return None; }
    let guild_id = msg.guild_id?;
    let guild_cfg = db.get_guild(guild_id.get()).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id.get()));

    let nsfw_threshold = guild_cfg.nsfw_threshold.unwrap_or(global_config.nsfw_threshold);
    let gore_threshold = guild_cfg.gore_threshold.unwrap_or(global_config.gore_threshold);
    // Review thresholds are always lower than block — if not overridden, derive from global
    let nsfw_review = guild_cfg.nsfw_threshold.map(|t| (t - 0.10).max(0.0)).unwrap_or(global_config.nsfw_review_threshold);
    let gore_review = guild_cfg.gore_threshold.map(|t| (t - 0.10).max(0.0)).unwrap_or(global_config.gore_review_threshold);
    let block_gender = guild_cfg.block_gender_images.unwrap_or(global_config.block_gender_images);
    let block_invites = guild_cfg.block_invites.unwrap_or(global_config.block_invites);
    let log_channel = guild_cfg.log_channel_id.or(global_config.log_channel_id);
    let delete = guild_cfg.delete_message.unwrap_or(global_config.delete_message);
    let warn_user = guild_cfg.send_warning.unwrap_or(global_config.send_warning);
    let timeout_secs = guild_cfg.timeout_duration_seconds.unwrap_or(global_config.timeout_duration_seconds);
    let enable_review_queue = global_config.enable_review_queue;

    let is_nsfw_channel = ctx.cache.guild_channel(msg.channel_id).map(|c| c.nsfw).unwrap_or(false);
    if let Some(member) = &msg.member {
        if let Some(reason) = permissions::is_exempt(&member.roles, msg.channel_id.get(), &guild_cfg, is_nsfw_channel) {
            info!("Exempt {}: {}", msg.author.tag(), reason);
            return None;
        }
    } else if guild_cfg.exempt_channels.contains(&msg.channel_id.get()) {
        info!("Exempt channel {} for {}", msg.channel_id, msg.id);
        return None;
    }
    if guild_cfg.allow_in_nsfw_channels.unwrap_or(global_config.allow_in_nsfw_channels) && is_nsfw_channel {
        return None;
    }

    // Merged domains
    let mut blocked = global_config.blocked_domains.clone();
    blocked.extend(guild_cfg.blocked_domains.clone());
    let mut allowed = global_config.allowed_domains.clone();
    allowed.extend(guild_cfg.allowed_domains.clone());
    let effective_scanner = LinkScanner::new(blocked, allowed, block_invites);

    // Collect image URLs — do NOT add attachment CDN URLs to link scan text (prevents false link flag)
    let mut image_urls: Vec<String> = Vec::new();
    let mut text_to_scan = msg.content.clone();
    for att in &msg.attachments {
        let url = att.url.clone();
        let is_img = att.content_type.as_ref().map(|ct| ct.starts_with("image/")).unwrap_or(false) || is_likely_image_url(&url);
        if is_img { image_urls.push(url); }
        // Note: we do NOT push attachment URL into text_to_scan — avoids flagging CDN as link
    }
    for embed in &msg.embeds {
        if let Some(url) = &embed.url { text_to_scan.push(' '); text_to_scan.push_str(url); }
        if let Some(img) = &embed.image { image_urls.push(img.url.clone()); }
        if let Some(thumb) = &embed.thumbnail { image_urls.push(thumb.url.clone()); }
    }
    if !msg.sticker_items.is_empty() { for s in &msg.sticker_items { warn!("Sticker {} in {} — TODO scan", s.id, msg.id); } }
    for u in link_scanner.extract_urls(&msg.content) { if is_likely_image_url(&u) { image_urls.push(u); } }
    let mut seen = HashSet::new();
    image_urls.retain(|u| seen.insert(u.clone()));

    // Link scan — now with builtin safe-list + CDN exemption
    let link_results = effective_scanner.scan_text(&text_to_scan);
    let blocked_links: Vec<_> = link_results.iter().filter(|r| r.blocked).cloned().collect();

    // Image scan with tiered logic + safe-hash bypass
    let mut blocked_images = Vec::new();
    let mut review_images = Vec::new();
    for url in &image_urls {
        let res = image_filter.scan_url(url, http_client, |h| db.is_safe_hash(h)).await;
        if let Some(err) = &res.error {
            warn!("Image error {}: {}", url, err);
            continue;
        }
        // Re-evaluate against effective per-guild thresholds (image_filter uses global)
        // We have raw scores in res; recompute tiers here for accuracy
        let nsfw_tier = if res.nsfw_score >= nsfw_threshold { Tier::Block } else if res.nsfw_score >= nsfw_review { Tier::Review } else { Tier::Allow };
        let gore_tier = if res.gore_score >= gore_threshold { Tier::Block } else if res.gore_score >= gore_review { Tier::Review } else { Tier::Allow };
        let tier = match (nsfw_tier, gore_tier) {
            (Tier::Block, _) | (_, Tier::Block) => Tier::Block,
            (Tier::Review, _) | (_, Tier::Review) => Tier::Review,
            _ => Tier::Allow,
        };
        // Gender never blocks in stub — keep Allow

        match tier {
            Tier::Block => {
                let mut r = res.clone();
                r.blocked = true; r.review = false; r.tier = Tier::Block;
                // Ensure reason reflects effective threshold
                if r.reasons.iter().any(|s| s.contains("skipped")) { continue; }
                blocked_images.push(r);
            }
            Tier::Review => {
                if enable_review_queue {
                    let mut r = res.clone();
                    r.blocked = false; r.review = true; r.tier = Tier::Review;
                    review_images.push(r);
                }
            }
            Tier::Allow => {}
        }
        // If original res was already blocked due to global threshold, preserve it
        if res.blocked && !blocked_images.iter().any(|b| b.url == res.url) && !review_images.iter().any(|b| b.url == res.url) {
            blocked_images.push(res);
        }
    }

    let mut violations: Vec<String> = blocked_links.iter().map(|bl| format!("🔗 Blocked link `{}` — {}", bl.url, bl.reason)).collect();
    violations.extend(blocked_images.iter().map(|ir| format!("🖼️ Blocked image <{}> — {} (hash `{}`)", ir.url, ir.reasons.join(", "), &ir.hash[..12.min(ir.hash.len())])));

    let mut reviews: Vec<String> = review_images.iter().map(|ir| format!("🔍 Review image <{}> — {} (hash `{}`)", ir.url, ir.reasons.join(", "), &ir.hash[..12.min(ir.hash.len())])).collect();
    // Link review tier not yet needed — links are binary (safe-list handles false positives)

    if violations.is_empty() && reviews.is_empty() { return None; }

    if !violations.is_empty() {
        info!(guild=%guild_id, channel=%msg.channel_id, author=%msg.author.tag(), "Block: {:?}", violations);
    }
    if !reviews.is_empty() {
        info!(guild=%guild_id, channel=%msg.channel_id, author=%msg.author.tag(), "Review: {:?}", reviews);
    }

    // Logging — distinct embeds for block vs review
    if let Some(log_channel_id) = log_channel {
        let log_id = ChannelId::new(log_channel_id);
        if !violations.is_empty() {
            let description = violations.join("\n");
            let footer_text = format!("User: {} ({}) | Action: BLOCK", msg.author.tag(), msg.author.id);
            let embed = CreateEmbed::new().title("Rosy Guard — violation blocked").description(&description).footer(CreateEmbedFooter::new(footer_text)).field("Channel", format!("<#{}>", msg.channel_id), true).field("Message ID", msg.id.to_string(), true).field("Scores", format!("NSFW {:.2}≥{:.2} | Gore {:.2}≥{:.2}", blocked_images.first().map(|r| r.nsfw_score).unwrap_or(0.0), nsfw_threshold, blocked_images.first().map(|r| r.gore_score).unwrap_or(0.0), gore_threshold), false).color(0xFF2D55);
            let content = format!("**Violation** by {} in <#{}> — {}\n*If false positive, mod can run `/rosy false_positive` to allow hash.*", msg.author.mention(), msg.channel_id, description);
            if let Err(e) = log_id.send_message(&ctx.http, CreateMessage::new().content(content).embed(embed)).await { error!("Log failed: {:?}", e); }
        }
        if !reviews.is_empty() && enable_review_queue {
            let description = reviews.join("\n");
            let footer_text = format!("User: {} ({}) | Action: REVIEW (not deleted)", msg.author.tag(), msg.author.id);
            let embed = CreateEmbed::new().title("Rosy Guard — review queue").description(&description).footer(CreateEmbedFooter::new(footer_text)).field("Channel", format!("<#{}>", msg.channel_id), true).field("Message ID", msg.id.to_string(), true).field("Scores", format!("Review threshold NSFW {:.2} | Gore {:.2}", nsfw_review, gore_review), false).color(0xFFCC02);
            let content = format!("**Needs review** by {} in <#{}> — message kept, please check. Approve with `/rosy approve_hash` or allow hash.", msg.author.mention(), msg.channel_id);
            if let Err(e) = log_id.send_message(&ctx.http, CreateMessage::new().content(content).embed(embed)).await { error!("Review log failed: {:?}", e); }
            // Persist pending review
            let scores_json = serde_json::to_string(&reviews).unwrap_or_default();
            db.add_pending_review(guild_id.get(), msg.channel_id.get(), msg.author.id.get(), msg.id.get(), &reviews.join("; "), &scores_json);
        }
    }

    // Only auto-delete for high-confidence BLOCK tier
    if !violations.is_empty() && delete {
        if let Err(e) = msg.delete(&ctx.http).await { error!("Delete {} failed: {:?}", msg.id, e); } else { info!("Deleted {}", msg.id); db.log_action(guild_id.get(), msg.channel_id.get(), msg.author.id.get(), "delete", &violations.join("; ")); }
    }

    if !violations.is_empty() && warn_user {
        let warning = format!("{}, your message was removed (high confidence violation):\n{}\nIf this is wrong, please contact a mod — they can `/rosy false_positive` to allow it.", msg.author.mention(), violations.join("\n"));
        if let Err(e) = msg.channel_id.send_message(&ctx.http, CreateMessage::new().content(warning)).await { warn!("Warn failed: {:?}", e); }
    }

    if !violations.is_empty() && timeout_secs > 0 {
        let until = chrono::Utc::now() + chrono::Duration::seconds(timeout_secs as i64);
        if let Err(e) = guild_id.edit_member(&ctx.http, msg.author.id, serenity::builder::EditMember::new().disable_communication_until_datetime(&until.into())).await { warn!("Timeout {} failed: {:?}", msg.author.tag(), e); } else { info!("Timed out {} until {}", msg.author.tag(), until.format("%Y-%m-%d %H:%M:%S UTC")); db.log_action(guild_id.get(), msg.channel_id.get(), msg.author.id.get(), "timeout", &format!("{}s: {}", timeout_secs, violations.join("; "))); }
    }

    Some(ScanOutcome { violations, reviews, image_results: blocked_images, link_results: blocked_links })
}

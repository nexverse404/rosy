//! Message handling — tiered safety + NSFW/Gore/Tor/Sus + video + per-channel gender

use std::{collections::HashSet, sync::Arc};
use serenity::all::{ChannelId, CreateMessage, Message};
use serenity::prelude::Context;
use tracing::{error, info, warn};

use crate::{config::{Config, Tier}, db::Db, embeds, moderation::{image_filter::ImageFilter, link_scanner::LinkScanner}, permissions};

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

pub fn is_video_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.ends_with(".mp4") || lower.ends_with(".webm") || lower.ends_with(".mov") || lower.ends_with(".m4v") || lower.ends_with(".avi") || lower.ends_with(".mkv") || lower.ends_with(".flv") || lower.ends_with(".3gp")
        || lower.ends_with(".mpg") || lower.ends_with(".mpeg") || lower.ends_with(".wmv")
        || lower.contains("cdn.discordapp.com/attachments") && (lower.contains(".mp4") || lower.contains(".mov") || lower.contains(".webm"))
}

pub fn is_video_attachment(att: &serenity::all::Attachment) -> bool {
    if let Some(ct) = &att.content_type {
        if ct.starts_with("video/") { return true; }
    }
    is_video_url(&att.url) || is_video_url(&att.filename)
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
    let nsfw_review = guild_cfg.nsfw_threshold.map(|t| (t - 0.10).max(0.0)).unwrap_or(global_config.nsfw_review_threshold);
    let gore_review = guild_cfg.gore_threshold.map(|t| (t - 0.10).max(0.0)).unwrap_or(global_config.gore_review_threshold);
    let log_channel = guild_cfg.log_channel_id.or(global_config.log_channel_id);
    let delete = guild_cfg.delete_message.unwrap_or(global_config.delete_message);
    let warn_user = guild_cfg.send_warning.unwrap_or(global_config.send_warning);
    let timeout_secs = guild_cfg.timeout_duration_seconds.unwrap_or(global_config.timeout_duration_seconds);
    let enable_review_queue = global_config.enable_review_queue;
    let channel_id_u64 = msg.channel_id.get();

    let is_nsfw_channel = ctx.cache.guild_channel(msg.channel_id).map(|c| c.nsfw).unwrap_or(false);
    if let Some(member) = &msg.member {
        if let Some(reason) = permissions::is_exempt(&member.roles, channel_id_u64, &guild_cfg, is_nsfw_channel) {
            info!("Exempt {}: {}", msg.author.tag(), reason);
            return None;
        }
    } else if guild_cfg.exempt_channels.contains(&channel_id_u64) {
        info!("Exempt channel {} for {}", msg.channel_id, msg.id);
        return None;
    }
    if guild_cfg.allow_in_nsfw_channels.unwrap_or(global_config.allow_in_nsfw_channels) && is_nsfw_channel { return None; }

    let mut allowed = global_config.allowed_domains.clone();
    allowed.extend(guild_cfg.allowed_domains.clone());

    // ── Collect image & video URLs ──
    let mut image_urls: Vec<String> = Vec::new();
    let mut video_urls: Vec<String> = Vec::new();
    let mut text_to_scan = msg.content.clone();

    for att in &msg.attachments {
        if is_video_attachment(att) {
            video_urls.push(att.url.clone());
        } else {
            let is_img = att.content_type.as_ref().map(|ct| ct.starts_with("image/")).unwrap_or(false) || is_likely_image_url(&att.url);
            if is_img { image_urls.push(att.url.clone()); }
            // Video URLs that aren't image but look like video
            if (is_video_url(&att.url) || att.content_type.as_ref().map(|ct| ct.starts_with("video/")).unwrap_or(false))
                && !video_urls.contains(&att.url) { video_urls.push(att.url.clone()); }
        }
    }
    for embed in &msg.embeds {
        if let Some(url) = &embed.url { text_to_scan.push(' '); text_to_scan.push_str(url); }
        if let Some(vid) = &embed.video { video_urls.push(vid.url.clone()); }
        // Some video embeds also have thumbnail that is image — still scan image separately
        if let Some(img) = &embed.image { image_urls.push(img.url.clone()); }
        if let Some(thumb) = &embed.thumbnail { image_urls.push(thumb.url.clone()); }
        // If embed url itself looks like video
        if let Some(url) = &embed.url { if is_video_url(url) { video_urls.push(url.clone()); } }
    }
    if !msg.sticker_items.is_empty() { for s in &msg.sticker_items { warn!("Sticker {} in {} — TODO scan", s.id, msg.id); } }
    for u in link_scanner.extract_urls(&msg.content) {
        if is_likely_image_url(&u) { image_urls.push(u.clone()); }
        if is_video_url(&u) { video_urls.push(u.clone()); }
    }
    // Also check for video links in content via simple detection
    // e.g. youtube, tiktok etc could be considered video embeds — but we only block actual video files/uploads for now
    // The recommendation: block all video embeds/uploads since not moderatable

    let mut seen_img = HashSet::new();
    image_urls.retain(|u| seen_img.insert(u.clone()));
    let mut seen_vid = HashSet::new();
    video_urls.retain(|u| seen_vid.insert(u.clone()));
    // Don't double-count same URL as both image and video
    // If URL is video, remove from image list
    if !video_urls.is_empty() {
        image_urls.retain(|u| !video_urls.contains(u));
    }

    let link_results = link_scanner.scan_text(&text_to_scan);
    let blocked_links: Vec<_> = link_results.iter().filter(|r| {
        if !r.blocked { return false; }
        if allowed.contains(&r.domain.to_ascii_lowercase()) { return false; }
        true
    }).cloned().collect();

    // ── Video blocking ──
    let should_block_videos = guild_cfg.blocks_videos_in(channel_id_u64, global_config.block_videos);
    let mut blocked_videos: Vec<String> = Vec::new();
    if should_block_videos && !video_urls.is_empty() {
        for vurl in &video_urls {
            blocked_videos.push(format!("🎞️ <{vurl}> — Video blocked (not moderatable yet — recommended). *Admins: `/rosy video enable:false` or per-channel `/rosy video channel:#ch enable:false` to allow*"));
        }
        info!(guild=%guild_id, channel=%msg.channel_id, author=%msg.author.tag(), "Video block: {} videos in {}", video_urls.len(), channel_id_u64);
    }

    // ── Image scanning with per-channel gender ──
    let channel_blocks_male = guild_cfg.blocks_male_in(channel_id_u64);
    let channel_blocks_female = guild_cfg.blocks_female_in(channel_id_u64);

    let mut blocked_images = Vec::new();
    let mut review_images = Vec::new();
    for url in &image_urls {
        // Use per-channel gender policy
        let res = image_filter.scan_url_with_gender_policy(url, http_client, |h| db.is_safe_hash(h), channel_blocks_male, channel_blocks_female).await;
        if let Some(err) = &res.error { warn!("Image error {}: {}", url, err); continue; }
        if res.reasons.iter().any(|s| s.contains("skipped")) { continue; }
        // Apply thresholds (NSFW/Gore) — image_filter already applied block thresholds at 0.85 global, but we respect per-guild
        let nsfw_tier = if res.nsfw_score >= nsfw_threshold { Tier::Block } else if res.nsfw_score >= nsfw_review { Tier::Review } else { Tier::Allow };
        let gore_tier = if res.gore_score >= gore_threshold { Tier::Block } else if res.gore_score >= gore_review { Tier::Review } else { Tier::Allow };
        // Gender already decided per-channel via scan_url_with_gender_policy (res.gender_blocked)
        let gender_block = res.gender_blocked;
        let tier = if gender_block { Tier::Block } else { match (nsfw_tier, gore_tier) { (Tier::Block, _) | (_, Tier::Block) => Tier::Block, (Tier::Review, _) | (_, Tier::Review) => Tier::Review, _ => Tier::Allow } };
        match tier {
            Tier::Block => { let mut r = res.clone(); r.blocked = true; r.review = false; r.tier = Tier::Block; blocked_images.push(r); }
            Tier::Review => if enable_review_queue { let mut r = res.clone(); r.blocked = false; r.review = true; r.tier = Tier::Review; review_images.push(r); }
            Tier::Allow => {}
        }
        // Fallback: if original model said blocked but our tier said allow due to higher thresholds, respect original if gender
        if res.blocked && !blocked_images.iter().any(|b| b.url == res.url) && !review_images.iter().any(|b| b.url == res.url) {
            // Only keep if it was gender-based and channel blocks that gender
            if res.gender_blocked { blocked_images.push(res); }
        }
    }

    // Build violations + categories for embed coloring
    let mut violations: Vec<String> = Vec::new();
    let mut v_categories: Vec<String> = Vec::new();
    for bl in &blocked_links {
        let cat = bl.category.clone().unwrap_or_else(|| "NSFW/Gore".into());
        if !v_categories.contains(&cat) { v_categories.push(cat.clone()); }
        violations.push(format!("`{}` — {} `({})`", bl.domain, bl.reason, cat));
    }
    for v in &blocked_videos {
        if !v_categories.contains(&"Video".to_string()) { v_categories.push("Video".to_string()); }
        violations.push(v.clone());
    }
    for ir in &blocked_images {
        let cat = if ir.gender_blocked {
            if ir.gender.male >= 0.90 || ir.gender.predicted == crate::moderation::image_filter::Gender::Male { "Gender/Male" } else if ir.gender.female >= 0.90 { "Gender/Female" } else { "Gender" }
        } else if ir.nsfw_score >= nsfw_threshold { "NSFW" } else if ir.gore_score >= gore_threshold { "Gore" } else { "NSFW/Gore" };
        if !v_categories.contains(&cat.to_string()) { v_categories.push(cat.to_string()); }
        violations.push(format!("🖼️ <{}> — {} `hash:{}`", ir.url, ir.reasons.join(", "), &ir.hash[..12.min(ir.hash.len())]));
    }
    let reviews: Vec<String> = review_images.iter().map(|ir| format!("🔍 <{}> — {} `hash:{}`", ir.url, ir.reasons.join(", "), &ir.hash[..12.min(ir.hash.len())])).collect();
    let r_categories = vec!["Review".to_string()];

    if violations.is_empty() && reviews.is_empty() { return None; }
    if !violations.is_empty() { info!(guild=%guild_id, channel=%msg.channel_id, author=%msg.author.tag(), "Block: {:?}", violations); }
    if !reviews.is_empty() { info!(guild=%guild_id, channel=%msg.channel_id, author=%msg.author.tag(), "Review: {:?}", reviews); }

    let bot_avatar = ctx.cache.current_user().avatar_url();

    if let Some(log_channel_id) = log_channel {
        let log_id = ChannelId::new(log_channel_id);
        if !violations.is_empty() {
            let scores_line = blocked_images.first().map(|r| format!("NSFW {:.2}≥{:.2} • Gore {:.2}≥{:.2} • Tor/Proxy via hagezi • Cheap-TLD via hagezi/spam-tlds • Video: blocked (recommended) • Gender: per-channel", r.nsfw_score, nsfw_threshold, r.gore_score, gore_threshold));
            let embed = embeds::block_embed(&msg.author, format!("<#{}>", msg.channel_id), &violations, &v_categories, scores_line, bot_avatar.clone());
            let content = format!("**Violation** by {} in <#{chan}> • `{msg_id} • {time}`\n*Link filter: GitHub — `blocklistproject/Lists` (NSFW) + `aegis-blocklist` (Gore) + `hagezi/dns-blocklists` (Tor & Cheap-TLD). Video blocked as not moderatable (recommended). Gender per-channel. False positive?* `/rosy false_positive`", msg.author.mention(), chan=msg.channel_id, msg_id=msg.id, time=chrono::Utc::now().format("%H:%M UTC"));
            if let Err(e) = log_id.send_message(&ctx.http, CreateMessage::new().content(content).embed(embed)).await { error!("Log failed: {:?}", e); }
        }
        if !reviews.is_empty() && enable_review_queue {
            let embed = embeds::review_embed(&msg.author, format!("<#{}>", msg.channel_id), &reviews, &r_categories, bot_avatar.clone());
            let content = format!("**Needs review** by {} in <#{chan}> • `{msg_id} • {time}` — kept, please check.", msg.author.mention(), chan=msg.channel_id, msg_id=msg.id, time=chrono::Utc::now().format("%H:%M UTC"));
            if let Err(e) = log_id.send_message(&ctx.http, CreateMessage::new().content(content).embed(embed)).await { error!("Review log failed: {:?}", e); }
            let scores_json = serde_json::to_string(&reviews).unwrap_or_default();
            db.add_pending_review(guild_id.get(), msg.channel_id.get(), msg.author.id.get(), msg.id.get(), &reviews.join("; "), &scores_json);
        }
    }

    if !violations.is_empty() && delete {
        if let Err(e) = msg.delete(&ctx.http).await { error!("Delete {} failed: {:?}", msg.id, e); } else { info!("Deleted {}", msg.id); db.log_action(guild_id.get(), msg.channel_id.get(), msg.author.id.get(), "delete", &violations.join("; ")); }
    }
    if !violations.is_empty() && warn_user {
        let warn_embed = embeds::block_embed(&msg.author, format!("<#{}>", msg.channel_id), &violations, &v_categories, None, bot_avatar.clone());
        let warning = CreateMessage::new().content(format!("{} your message was removed — high-confidence violation detected.", msg.author.mention())).embed(warn_embed);
        if let Err(e) = msg.channel_id.send_message(&ctx.http, warning).await { warn!("Warn failed: {:?}", e); }
    }
    if !violations.is_empty() && timeout_secs > 0 {
        let until = chrono::Utc::now() + chrono::Duration::seconds(timeout_secs as i64);
        if let Err(e) = guild_id.edit_member(&ctx.http, msg.author.id, serenity::builder::EditMember::new().disable_communication_until_datetime(&until.into())).await { warn!("Timeout {} failed: {:?}", msg.author.tag(), e); } else { info!("Timed out {} until {}", msg.author.tag(), until.format("%Y-%m-%d %H:%M:%S UTC")); db.log_action(guild_id.get(), msg.channel_id.get(), msg.author.id.get(), "timeout", &format!("{}s: {}", timeout_secs, violations.join("; "))); }
    }

    Some(ScanOutcome { violations, reviews, image_results: blocked_images, link_results: blocked_links })
}

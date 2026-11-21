//! Beautiful embeds for Rosy Guard — category-colored, icon-rich, consistent branding

use serenity::all::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, Timestamp, User};

pub const COLOR_NSFW: u32 = 0xFF2D55; // vibrant rose red
pub const COLOR_GORE: u32 = 0x8B0000; // deep blood red
pub const COLOR_TOR: u32 = 0x7C3AED; // electric purple
pub const COLOR_SUS: u32 = 0xFF8C00; // amber orange
pub const COLOR_INVITE: u32 = 0x3B82F6; // sky blue
pub const COLOR_REVIEW: u32 = 0xFFCC02; // warm yellow
pub const COLOR_INFO: u32 = 0x57F287; // discord green

pub fn color_for_category(cat: &str) -> u32 {
    match cat.to_ascii_lowercase().as_str() {
        c if c.contains("nsfw") => COLOR_NSFW,
        c if c.contains("gore") => COLOR_GORE,
        c if c.contains("tor") || c.contains("proxy") || c.contains("onion") => COLOR_TOR,
        c if c.contains("sus") || c.contains("cheap") || c.contains("tld") || c.contains("new") => COLOR_SUS,
        c if c.contains("invite") => COLOR_INVITE,
        c if c.contains("review") => COLOR_REVIEW,
        _ => COLOR_NSFW,
    }
}

pub fn category_emoji(cat: &str) -> &'static str {
    match cat.to_ascii_lowercase().as_str() {
        c if c.contains("nsfw") => "🔞",
        c if c.contains("gore") => "🩸",
        c if c.contains("tor") || c.contains("proxy") => "🧅",
        c if c.contains("sus") || c.contains("cheap") || c.contains("tld") => "⚠️",
        c if c.contains("invite") => "📨",
        _ => "🛡️",
    }
}

/// Block embed — clean, beautiful, with per-category color grouping
pub fn block_embed(
    author_user: &User,
    channel_mention: String,
    violations: &[String],
    categories: &[String],
    scores_line: Option<String>,
    bot_avatar_url: Option<String>,
) -> CreateEmbed {
    // Dominant category determines color
    let dominant = categories.first().map(|s| s.as_str()).unwrap_or("NSFW");
    let color = color_for_category(dominant);
    let emoji = categories.iter().map(|c| category_emoji(c)).collect::<Vec<_>>().join(" ");

    let description = format!(
        "{emoji} **Violation detected** — {} item(s) blocked\n{}\n",
        violations.len(),
        violations.iter().map(|v| format!("• {}", v)).collect::<Vec<_>>().join("\n")
    );

    let mut embed = CreateEmbed::new()
        .title(format!("{} Rosy Guard — Blocked", emoji))
        .description(description)
        .color(color)
        .field("Channel", channel_mention, true)
        .field("Category", categories.join(", "), true)
        .field("Action", "🗑️ Deleted • ⚠️ Warned • ⏱️ Timeout", false);

    if let Some(scores) = scores_line {
        embed = embed.field("Detection", scores, false);
    }

    embed = embed
        .author(CreateEmbedAuthor::new(format!("Triggered by {}", author_user.tag())).icon_url(author_user.face()))
        .footer(CreateEmbedFooter::new("Rosy Guard • NSFW: blocklistproject/Lists • Gore: aegis-blocklist • Tor: hagezi • TLD: hagezi/spam-tlds").icon_url(bot_avatar_url.unwrap_or_else(|| "https://github.com/nexverse404/rosy/blob/main/.github/rose.png?raw=true".into())))
        .timestamp(Timestamp::now());

    embed
}

/// Review queue embed — yellow, non-deleted
pub fn review_embed(
    author_user: &User,
    channel_mention: String,
    reviews: &[String],
    categories: &[String],
    bot_avatar_url: Option<String>,
) -> CreateEmbed {
    let description = format!(
        "🔍 **Needs human review** — {} item(s) (medium confidence, not auto-deleted)\n{}\n",
        reviews.len(),
        reviews.iter().map(|v| format!("• {}", v)).collect::<Vec<_>>().join("\n")
    );
    CreateEmbed::new()
        .title("🔍 Rosy Guard — Review Queue")
        .description(description)
        .color(COLOR_REVIEW)
        .field("Channel", channel_mention, true)
        .field("Category", categories.join(", "), true)
        .field("Action", "👀 Kept — please review. Use `/rosy false_positive` if safe", false)
        .author(CreateEmbedAuthor::new(format!("Posted by {}", author_user.tag())).icon_url(author_user.face()))
        .footer(CreateEmbedFooter::new("Rosy Guard • Review threshold 0.75–0.85 • Approve with /rosy allow_hash").icon_url(bot_avatar_url.unwrap_or_else(|| "https://github.com/nexverse404/rosy/blob/main/.github/rose.png?raw=true".into())))
        .timestamp(Timestamp::now())
}

/// Simple status/info embed
pub fn info_embed(title: &str, description: &str) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description)
        .color(COLOR_INFO)
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Rosy Guard 🌹"))
}

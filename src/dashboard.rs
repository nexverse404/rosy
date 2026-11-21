//! Easy config dashboard — interactive buttons + presets + per-channel gender & video

use serenity::all::{
    CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuOption, Timestamp,
};
use tracing::info;

use crate::{config_presets::Preset, db::{Db, GuildConfig}, embeds};

pub fn build_dashboard_embed(guild_id: u64, cfg: &GuildConfig, global_nsfw: f32, global_gore: f32, filter_ns: usize, filter_gore: usize, filter_tor: usize, filter_tld: usize, global_block_videos: bool) -> CreateEmbed {
    let nsfw_thr = cfg.nsfw_threshold.map(|v| format!("{:.2}", v)).unwrap_or_else(|| format!("global {:.2}", global_nsfw));
    let gore_thr = cfg.gore_threshold.map(|v| format!("{:.2}", v)).unwrap_or_else(|| format!("global {:.2}", global_gore));
    let gender = cfg.gender_summary();
    let gender_detail = if cfg.gender_block_male_channels.is_empty() && cfg.gender_block_female_channels.is_empty() && cfg.gender_block_male.is_none() && cfg.gender_block_female.is_none() && cfg.block_gender_images.is_none() {
        "Off — no gender filtering".to_string()
    } else {
        format!("{} — per-channel allowed", gender)
    };
    let invites = if cfg.block_invites.unwrap_or(true) {"✅ On"} else {"❌ Off"};
    let review = "✅ On (0.75–0.85 → mod queue)";
    let allow_nsfw = if cfg.allow_in_nsfw_channels.unwrap_or(false) {"Yes"} else {"No"};
    let log = cfg.log_channel_id.map(|id| format!("<#{id}>")).unwrap_or_else(|| "Not set (uses global)".into());
    let exempt_roles = if cfg.exempt_roles.is_empty() {"None".into()} else { format!("{} role(s)", cfg.exempt_roles.len()) };
    let exempt_ch = if cfg.exempt_channels.is_empty() {"None".into()} else { format!("{} channel(s)", cfg.exempt_channels.len()) };
    let blocked = if cfg.blocked_domains.is_empty() {"None".into()} else { cfg.blocked_domains.iter().take(3).cloned().collect::<Vec<_>>().join(", ") + if cfg.blocked_domains.len() > 3 {", …"} else {""}};
    let allowed = if cfg.allowed_domains.is_empty() {"None".into()} else { cfg.allowed_domains.iter().take(3).cloned().collect::<Vec<_>>().join(", ") + if cfg.allowed_domains.len() > 3 {", …"} else {""}};
    let video = cfg.video_summary(global_block_videos);
    let video_detail = if cfg.block_videos.unwrap_or(global_block_videos) {
        format!("{} — ⚠️ recommended until video moderation lands", video)
    } else if !cfg.video_blocked_channels.is_empty() {
        format!("{} — blocks uploads/embeds in those channels", video)
    } else {
        format!("{} — video not blocked (enable recommended)", video)
    };

    CreateEmbed::new()
        .title("🌹 Rosy Guard — Easy Config Dashboard")
        .description(format!(
            "**Guild `{}` • One-click setup for admins**\n\
            > Use buttons below to toggle, or pick a preset — changes apply instantly. No restart needed.\n\
            > `Tip: /rosy preset balanced` is the recommended starting point.`",
            guild_id
        ))
        .color(0xFF2D55)
        .field("🔞 NSFW Threshold", format!("`{}` — block ≥thr, review thr-0.10", nsfw_thr), true)
        .field("🩸 Gore Threshold", format!("`{}` — same tiers", gore_thr), true)
        .field("🧬 Gender Filter", format!("`{}`\n_{}_", gender, gender_detail), true)
        .field("🎞️ Video Blocking", format!("`{}`\n_{}_", video, video_detail), true)
        .field("📨 Block Invites", invites, true)
        .field("🔓 Bypass in NSFW channels", allow_nsfw, true)
        .field("👀 Review Queue", review, true)
        .field("📢 Log Channel", log, true)
        .field("👑 Exempt Roles", exempt_roles, true)
        .field("💬 Exempt Channels", exempt_ch, true)
        .field("🚫 Extra Blocked", blocked, true)
        .field("✅ Allowlisted", allowed, true)
        .field("📚 Filterlists (live)", format!("NSFW `{}` • Gore `{}` • Tor `{}` • Cheap-TLD `{}`", filter_ns, filter_gore, filter_tor, filter_tld), false)
        .field("💡 Per-Channel Tips", "• `/rosy gender channel:#nsfw allow:male block:female` — per-channel male/female\n• `/rosy video channel:#clips enable:false` — allow video in one channel\n• Video is **recommended ON** globally — cannot be moderated for NSFW yet", false)
        .footer(CreateEmbedFooter::new("Rosy Guard • Changes save to SQLite • Use /rosy export to backup"))
        .timestamp(Timestamp::now())
        .author(CreateEmbedAuthor::new("Admin Dashboard").icon_url("https://cdn.discordapp.com/embed/avatars/0.png"))
}

pub fn dashboard_components(disabled: bool) -> Vec<CreateActionRow> {
    vec![
        CreateActionRow::Buttons(vec![
            CreateButton::new("rosy_toggle_nsfw").label("NSFW Filter").emoji('🔞'.into()).style(serenity::all::ButtonStyle::Primary).disabled(disabled),
            CreateButton::new("rosy_toggle_gore").label("Gore Filter").emoji('🩸'.into()).style(serenity::all::ButtonStyle::Danger).disabled(disabled),
            CreateButton::new("rosy_toggle_tor").label("Tor/Proxy").emoji('🧅'.into()).style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
            CreateButton::new("rosy_toggle_sus").label("Cheap-Sus TLD").emoji('⚠️'.into()).style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
        ]),
        CreateActionRow::Buttons(vec![
            CreateButton::new("rosy_toggle_invites").label("Invites").emoji('📨'.into()).style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
            CreateButton::new("rosy_toggle_nsfwch").label("Bypass NSFW-ch").emoji('🔓'.into()).style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
            CreateButton::new("rosy_preset_balanced").label("Preset: Balanced").emoji('⚖'.into()).style(serenity::all::ButtonStyle::Success).disabled(disabled),
            CreateButton::new("rosy_help").label("Help").emoji('❓'.into()).style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
        ]),
        // Gender + Video row
        CreateActionRow::Buttons(vec![
            CreateButton::new("rosy_toggle_gender_male").label("Male Filter").emoji('👨'.into()).style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
            CreateButton::new("rosy_toggle_gender_female").label("Female Filter").emoji('👩'.into()).style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
            CreateButton::new("rosy_toggle_video").label("Video Block").emoji('🎞'.into()).style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
            CreateButton::new("rosy_gender_help").label("Per-Channel").emoji('🎛'.into()).style(serenity::all::ButtonStyle::Primary).disabled(disabled),
        ]),
        CreateActionRow::SelectMenu(CreateSelectMenu::new(
            "rosy_threshold_select",
            serenity::all::CreateSelectMenuKind::String {
                options: vec![
                    CreateSelectMenuOption::new("NSFW 0.80 — Strict (more blocks)", "nsfw_0.80").description("Low threshold, catches more").emoji('🔞'.into()),
                    CreateSelectMenuOption::new("NSFW 0.85 — Balanced (recommended)", "nsfw_0.85").description("Default, good balance").emoji('⚖'.into()).default_selection(true),
                    CreateSelectMenuOption::new("NSFW 0.90 — Permissive (fewer false positives)", "nsfw_0.90").description("High threshold, fewer blocks").emoji('🌙'.into()),
                    CreateSelectMenuOption::new("Gore 0.80 — Strict", "gore_0.80").emoji('🩸'.into()),
                    CreateSelectMenuOption::new("Gore 0.85 — Balanced", "gore_0.85").emoji('⚖'.into()),
                    CreateSelectMenuOption::new("Gore 0.90 — Permissive", "gore_0.90").emoji('🌙'.into()),
                ],
            },
        ).placeholder("⚙️ Quick threshold presets — pick one").custom_id("rosy_threshold_select").disabled(disabled)),
        CreateActionRow::Buttons(vec![
            CreateButton::new("rosy_setup_wizard").label("🧭 Setup Wizard").style(serenity::all::ButtonStyle::Primary).disabled(disabled),
            CreateButton::new("rosy_export").label("💾 Export Config").style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
            CreateButton::new("rosy_refresh").label("🔄 Refresh").style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
        ]),
    ]
}

pub fn preset_components() -> Vec<CreateActionRow> {
    vec![CreateActionRow::SelectMenu(CreateSelectMenu::new(
        "rosy_preset_pick",
        serenity::all::CreateSelectMenuKind::String {
            options: vec![
                CreateSelectMenuOption::new("🛡️ Strict — max protection", "preset_strict").description("0.80 thr, Tor+Cheap on, 10m timeout, video on").emoji('🛡'.into()),
                CreateSelectMenuOption::new("⚖️ Balanced — recommended", "preset_balanced").description("0.85 thr, Tor+Cheap on, 5m timeout, video on").emoji('⚖'.into()),
                CreateSelectMenuOption::new("🌙 Permissive — minimal blocks", "preset_permissive").description("0.90 thr, Tor/Cheap off, no timeout, video off").emoji('🌙'.into()),
            ],
        },
    ).placeholder("Pick a preset to apply instantly").custom_id("rosy_preset_pick"))]
}

// Per-channel setup embed helper
pub fn per_channel_help_embed() -> CreateEmbed {
    CreateEmbed::new()
        .title("🎛️ Per-Channel Controls")
        .description("**Gender (male/female) & Video per-channel**\n\n\
            **Gender examples:**\n\
            • `/rosy gender channel:#general action:block gender:male` — block male images in #general\n\
            • `/rosy gender channel:#general action:allow gender:female` — allow female in that channel\n\
            • `/rosy gender channel:#general action:block gender:both` — block both\n\
            • `/rosy gender channel:#general action:clear` — clear channel override\n\
            • `/rosy gender action:block gender:female` — global female block\n\n\
            **Video examples (recommended ON until moderation exists):**\n\
            • `/rosy video enable:true` — block all videos server-wide (recommended)\n\
            • `/rosy video enable:false` — allow all videos server-wide\n\
            • `/rosy video channel:#clips enable:true` — block only in #clips\n\
            • `/rosy video channel:#clips enable:false` — allow in #clips when global is on\n\n\
            _Video blocks uploads + embeds (mp4/webm/mov etc). Cannot be moderated for NSFW yet — keeping it blocked is recommended._\n\
            _Gender uses 0.90 high-confidence threshold and requires `BLOCK_GENDER_IMAGES` model; per-channel keeps privacy scoped._")
        .color(0x57F287)
        .footer(CreateEmbedFooter::new("Rosy Guard • Tip: view per-channel summary in /rosy dashboard"))
}

// Helper to send dashboard
pub async fn send_dashboard(
    ctx: &serenity::prelude::Context,
    guild_id: u64,
    db: &Db,
    filter_counts: (usize, usize, usize, usize),
    global_nsfw: f32,
    global_gore: f32,
    global_block_videos: bool,
    target: serenity::all::ChannelId,
    as_followup: bool,
    interaction: Option<&serenity::all::CommandInteraction>,
) {
    let cfg = db.get_guild(guild_id).unwrap_or_else(|_| GuildConfig::new(guild_id));
    let embed = build_dashboard_embed(guild_id, &cfg, global_nsfw, global_gore, filter_counts.0, filter_counts.1, filter_counts.2, filter_counts.3, global_block_videos);
    let components = dashboard_components(false);
    if let Some(cmd) = interaction {
        let _ = cmd.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed).components(components).ephemeral(false)
        )).await;
    } else {
        let _ = target.send_message(&ctx.http, serenity::all::CreateMessage::new().embed(embed).components(components)).await;
    }
}

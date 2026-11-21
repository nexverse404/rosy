//! Easy config dashboard for admins — interactive buttons + selects + presets
//! One command `/rosy dashboard` opens a beautiful, click-to-configure panel.

use serenity::all::{
    CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuOption, Timestamp,
};
use tracing::info;

use crate::{config_presets::Preset, db::{Db, GuildConfig}, embeds};

fn cfg_line(cfg: &GuildConfig, global_nsfw: f32, global_gore: f32) -> String {
    let nsfw = cfg.nsfw_threshold.map(|v| format!("{:.2}", v)).unwrap_or_else(|| format!("global {:.2}", global_nsfw));
    let gore = cfg.gore_threshold.map(|v| format!("{:.2}", v)).unwrap_or_else(|| format!("global {:.2}", global_gore));
    let block_inv = cfg.block_invites.map(|b| if b {"on"} else {"off"}).unwrap_or("global");
    let allow_nsfw_ch = cfg.allow_in_nsfw_channels.map(|b| if b {"yes"} else {"no"}).unwrap_or("no");
    let log_ch = cfg.log_channel_id.map(|id| format!("<#{id}>")).unwrap_or_else(|| "global".into());
    format!("`NSFW {}` • `Gore {}` • `Invites {}` • `NSFW-ch bypass {}` • `Log {}`", nsfw, gore, block_inv, allow_nsfw_ch, log_ch)
}

pub fn build_dashboard_embed(guild_id: u64, cfg: &GuildConfig, global_nsfw: f32, global_gore: f32, filter_ns: usize, filter_gore: usize, filter_tor: usize, filter_tld: usize) -> CreateEmbed {
    let nsfw_thr = cfg.nsfw_threshold.map(|v| format!("{:.2}", v)).unwrap_or_else(|| format!("global {:.2}", global_nsfw));
    let gore_thr = cfg.gore_threshold.map(|v| format!("{:.2}", v)).unwrap_or_else(|| format!("global {:.2}", global_gore));
    let gender = if cfg.block_gender_images.unwrap_or(false) {"✅ On (0.90)"} else {"❌ Off"};
    let invites = if cfg.block_invites.unwrap_or(true) {"✅ On"} else {"❌ Off"};
    let review = "✅ On (0.75–0.85 → mod queue)";
    let allow_nsfw = if cfg.allow_in_nsfw_channels.unwrap_or(false) {"Yes"} else {"No"};
    let log = cfg.log_channel_id.map(|id| format!("<#{id}>")).unwrap_or_else(|| "Not set (uses global)".into());
    let exempt_roles = if cfg.exempt_roles.is_empty() {"None".into()} else { format!("{} role(s)", cfg.exempt_roles.len()) };
    let exempt_ch = if cfg.exempt_channels.is_empty() {"None".into()} else { format!("{} channel(s)", cfg.exempt_channels.len()) };
    let blocked = if cfg.blocked_domains.is_empty() {"None".into()} else { cfg.blocked_domains.iter().take(3).cloned().collect::<Vec<_>>().join(", ") + if cfg.blocked_domains.len() > 3 {", …"} else {""}};
    let allowed = if cfg.allowed_domains.is_empty() {"None".into()} else { cfg.allowed_domains.iter().take(3).cloned().collect::<Vec<_>>().join(", ") + if cfg.allowed_domains.len() > 3 {", …"} else {""}};

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
        .field("🧬 Gender Filter", gender, true)
        .field("📨 Block Invites", invites, true)
        .field("👀 Review Queue", review, true)
        .field("🔓 Bypass in NSFW channels", allow_nsfw, true)
        .field("📢 Log Channel", log, true)
        .field("👑 Exempt Roles", exempt_roles, true)
        .field("💬 Exempt Channels", exempt_ch, true)
        .field("🚫 Extra Blocked", blocked, true)
        .field("✅ Allowlisted", allowed, true)
        .field("📚 Filterlists (live)", format!("NSFW `{}` • Gore `{}` • Tor `{}` • Cheap-TLD `{}`", filter_ns, filter_gore, filter_tor, filter_tld), false)
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
            CreateButton::new("rosy_refresh").label("🔄 Refresh Dashboard").style(serenity::all::ButtonStyle::Secondary).disabled(disabled),
        ]),
    ]
}

pub fn preset_components() -> Vec<CreateActionRow> {
    vec![CreateActionRow::SelectMenu(CreateSelectMenu::new(
        "rosy_preset_pick",
        serenity::all::CreateSelectMenuKind::String {
            options: vec![
                CreateSelectMenuOption::new("🛡️ Strict — max protection", "preset_strict").description("0.80 thr, Tor+Cheap on, 10m timeout").emoji('🛡'.into()),
                CreateSelectMenuOption::new("⚖️ Balanced — recommended", "preset_balanced").description("0.85 thr, Tor+Cheap on, 5m timeout, review on").emoji('⚖'.into()),
                CreateSelectMenuOption::new("🌙 Permissive — minimal blocks", "preset_permissive").description("0.90 thr, Tor/Cheap off, no timeout").emoji('🌙'.into()),
            ],
        },
    ).placeholder("Pick a preset to apply instantly").custom_id("rosy_preset_pick"))]
}

// Helper to send dashboard
pub async fn send_dashboard(
    ctx: &serenity::prelude::Context,
    guild_id: u64,
    db: &Db,
    filter_counts: (usize, usize, usize, usize),
    global_nsfw: f32,
    global_gore: f32,
    target: serenity::all::ChannelId,
    as_followup: bool,
    interaction: Option<&serenity::all::CommandInteraction>,
) {
    let cfg = db.get_guild(guild_id).unwrap_or_else(|_| GuildConfig::new(guild_id));
    let embed = build_dashboard_embed(guild_id, &cfg, global_nsfw, global_gore, filter_counts.0, filter_counts.1, filter_counts.2, filter_counts.3);
    let components = dashboard_components(false);
    if let Some(cmd) = interaction {
        let _ = cmd.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed).components(components).ephemeral(false)
        )).await;
    } else {
        let _ = target.send_message(&ctx.http, serenity::all::CreateMessage::new().embed(embed).components(components)).await;
    }
}

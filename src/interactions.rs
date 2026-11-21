//! Component interaction handler — dashboard buttons + per-channel gender/video

use serenity::all::{
    ComponentInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use tracing::info;

use crate::{config_presets::Preset, db::Db, dashboard};

pub async fn handle_component(
    ctx: &serenity::prelude::Context,
    interaction: &ComponentInteraction,
    db: &Db,
) {
    let guild_id = match interaction.guild_id { Some(g) => g.get(), None => { respond(ctx, interaction, "Only in a server.").await; return; } };
    let is_admin = if let Some(member) = &interaction.member {
        if let Some(perms) = member.permissions { perms.manage_guild() || perms.administrator() } else { true }
    } else { false };
    if !is_admin {
        respond(ctx, interaction, "❌ Need **Manage Server** to use dashboard buttons.").await;
        return;
    }

    let custom = interaction.data.custom_id.as_str();
    match custom {
        "rosy_toggle_nsfw" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let cur = cfg.nsfw_threshold.unwrap_or(0.85);
            let new = if cur >= 1.0 { 0.85 } else { 1.0 };
            cfg.nsfw_threshold = Some(new);
            let _ = db.upsert_guild(&cfg);
            update_dashboard(ctx, interaction, db, guild_id, &format!("🔞 NSFW → {}", if new >= 1.0 {"OFF"} else {"ON (0.85)"})).await;
        }
        "rosy_toggle_gore" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let cur = cfg.gore_threshold.unwrap_or(0.85);
            let new = if cur >= 1.0 { 0.85 } else { 1.0 };
            cfg.gore_threshold = Some(new);
            let _ = db.upsert_guild(&cfg);
            update_dashboard(ctx, interaction, db, guild_id, &format!("🩸 Gore → {}", if new >= 1.0 {"OFF"} else {"ON (0.85)"})).await;
        }
        "rosy_toggle_tor" | "rosy_toggle_sus" => {
            respond(ctx, interaction, "🧅/⚠️ Tor & Cheap-TLD are always on via GitHub filterlists (hagezi). Use `/rosy allow_domain` to exempt a specific domain if needed.").await;
        }
        "rosy_toggle_invites" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let cur = cfg.block_invites.unwrap_or(true);
            cfg.block_invites = Some(!cur);
            let _ = db.upsert_guild(&cfg);
            update_dashboard(ctx, interaction, db, guild_id, &format!("📨 Invites → {}", if !cur {"ON"} else {"OFF"})).await;
        }
        "rosy_toggle_nsfwch" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let cur = cfg.allow_in_nsfw_channels.unwrap_or(false);
            cfg.allow_in_nsfw_channels = Some(!cur);
            let _ = db.upsert_guild(&cfg);
            update_dashboard(ctx, interaction, db, guild_id, &format!("🔓 Bypass NSFW-ch → {}", if !cur {"ON"} else {"OFF"})).await;
        }
        "rosy_toggle_gender_male" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let cur = cfg.gender_block_male.unwrap_or(false);
            cfg.gender_block_male = Some(!cur);
            let _ = db.upsert_guild(&cfg);
            update_dashboard(ctx, interaction, db, guild_id, &format!("👨 Male filter → {}", if !cur {"ON (global)"} else {"OFF"})).await;
        }
        "rosy_toggle_gender_female" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let cur = cfg.gender_block_female.unwrap_or(false);
            cfg.gender_block_female = Some(!cur);
            let _ = db.upsert_guild(&cfg);
            update_dashboard(ctx, interaction, db, guild_id, &format!("👩 Female filter → {}", if !cur {"ON (global)"} else {"OFF"})).await;
        }
        "rosy_toggle_video" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let global = crate::config::Config::from_env();
            let cur = cfg.block_videos.unwrap_or(global.block_videos);
            cfg.block_videos = Some(!cur);
            let _ = db.upsert_guild(&cfg);
            let toast = if !cur { "🎞️ Video blocking → ON (recommended: not moderatable yet)" } else { "🎞️ Video blocking → OFF (not recommended)" };
            update_dashboard(ctx, interaction, db, guild_id, toast).await;
        }
        "rosy_gender_help" => {
            let embed = dashboard::per_channel_help_embed();
            let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed).ephemeral(true)
            )).await;
        }
        "rosy_preset_balanced" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            Preset::Balanced.apply_to(&mut cfg);
            let _ = db.upsert_guild(&cfg);
            update_dashboard(ctx, interaction, db, guild_id, "⚖️ Preset → Balanced (0.85, Tor+Cheap on, video ON, 5m timeout)").await;
        }
        "rosy_help" => {
            let embed = CreateEmbed::new()
                .title("❓ Rosy Guard — Easy Setup Help")
                .description("**1️⃣ Pick a preset:** `/rosy preset balanced` (recommended)\n**2️⃣ Set log channel:** `/rosy log_channel channel:#mod-logs`\n**3️⃣ Fine-tune:** use buttons above or `/rosy quick` / `/rosy threshold` / `/rosy gender` / `/rosy video`\n**4️⃣ Dashboard:** `/rosy dashboard` re-opens this panel\n\n*Presets: Strict (0.80), Balanced (0.85), Permissive (0.90).* *Video recommended ON.*\n*Per-channel: `/rosy gender channel:#general action:block gender:male` — `/rosy video channel:#clips enable:false`*")
                .color(0x57F287)
                .footer(CreateEmbedFooter::new("Tip: /rosy export to backup your config as JSON"));
            let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed).ephemeral(true)
            )).await;
        }
        "rosy_setup_wizard" => {
            let embed = CreateEmbed::new()
                .title("🧭 Setup Wizard — 3 Steps")
                .description("**Step 1:** `/rosy log_channel channel:#mod-logs`\n**Step 2:** Click **Preset: Balanced** below\n**Step 3:** Test with `/rosy_test url:https://example.com`\n\n*Gender per-channel:* `/rosy gender channel:#nsfw action:block gender:female`\n*Video (recommended ON):* `/rosy video enable:true`\n*Need exemptions?* `/rosy exempt_role` / `exempt_channel` — trusted roles bypass all checks.\n*Too strict?* `/rosy threshold filter:nsfw value:0.90`")
                .color(0xFFCC02)
                .footer(CreateEmbedFooter::new("Rosy Guard • Setup takes ~60s"));
            let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed).ephemeral(true)
            )).await;
        }
        "rosy_export" => {
            let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let json = serde_json::to_string_pretty(&cfg).unwrap_or_else(|_| "{}".into());
            let embed = CreateEmbed::new()
                .title("💾 Config Export")
                .description(format!("```json\n{}\n```\nGuild `{}` — copy & save this JSON. Includes per-channel gender/video.", &json[..json.len().min(3500)], guild_id))
                .color(0x57F287)
                .footer(CreateEmbedFooter::new("Use /rosy preset to restore defaults, or edit JSON and re-import via dashboard (coming soon)"));
            let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed).ephemeral(true)
            )).await;
        }
        "rosy_refresh" => {
            update_dashboard(ctx, interaction, db, guild_id, "🔄 Refreshed").await;
        }
        _ => {
            if custom == "rosy_threshold_select" {
                if let serenity::all::ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind {
                    if let Some(v) = values.first() {
                        let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
                        match v.as_str() {
                            "nsfw_0.80" => cfg.nsfw_threshold = Some(0.80),
                            "nsfw_0.85" => cfg.nsfw_threshold = Some(0.85),
                            "nsfw_0.90" => cfg.nsfw_threshold = Some(0.90),
                            "gore_0.80" => cfg.gore_threshold = Some(0.80),
                            "gore_0.85" => cfg.gore_threshold = Some(0.85),
                            "gore_0.90" => cfg.gore_threshold = Some(0.90),
                            _ => {}
                        }
                        let _ = db.upsert_guild(&cfg);
                        update_dashboard(ctx, interaction, db, guild_id, &format!("🎚️ Threshold → {}", v)).await;
                        return;
                    }
                }
            } else if custom == "rosy_preset_pick" {
                if let serenity::all::ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind {
                    if let Some(v) = values.first() {
                        let preset = match v.as_str() {
                            "preset_strict" => Preset::Strict,
                            "preset_balanced" => Preset::Balanced,
                            "preset_permissive" => Preset::Permissive,
                            _ => Preset::Balanced,
                        };
                        let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
                        preset.apply_to(&mut cfg);
                        let _ = db.upsert_guild(&cfg);
                        update_dashboard(ctx, interaction, db, guild_id, &format!("{} Preset → {}", preset.emoji(), preset.as_str())).await;
                        return;
                    }
                }
            }
            respond(ctx, interaction, "Unknown button.").await;
        }
    }
}

async fn update_dashboard(
    ctx: &serenity::prelude::Context,
    interaction: &ComponentInteraction,
    db: &Db,
    guild_id: u64,
    toast: &str,
) {
    let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
    let global = crate::config::Config::from_env();
    let fl = crate::filterlist::FilterLists::load_bundled();
    let embed = dashboard::build_dashboard_embed(guild_id, &cfg, global.nsfw_threshold, global.gore_threshold, fl.nsfw_count, fl.gore_count, fl.tor_count, fl.cheap_tlds_count, global.block_videos);
    let components = dashboard::dashboard_components(false);
    let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::UpdateMessage(
        serenity::all::CreateInteractionResponseMessage::new().embed(embed).components(components)
    )).await;
    let _ = interaction.create_followup(&ctx.http, serenity::all::CreateInteractionResponseFollowup::new().content(format!("✅ {}", toast)).ephemeral(true)).await;
    info!("Guild {} dashboard action: {}", guild_id, toast);
}

async fn respond(ctx: &serenity::prelude::Context, interaction: &ComponentInteraction, content: &str) {
    let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().content(content).ephemeral(true)
    )).await;
}

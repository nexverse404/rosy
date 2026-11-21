//! Slash commands — /rosy config, status, allow/block, exempt

use serenity::all::{
    Command, CommandOptionType, CreateCommand, CreateCommandOption, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use tracing::{error, info};

use crate::db::Db;

pub fn register_commands() -> Vec<CreateCommand> {
    vec![
        CreateCommand::new("rosy")
            .description("Rosy Guard moderation controls")
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "status", "Show current filter status")
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "config", "Change thresholds and behaviour")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Number, "nsfw_threshold", "0.0–1.0").min_number_value(0.0).max_number_value(1.0).required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Number, "gore_threshold", "0.0–1.0").min_number_value(0.0).max_number_value(1.0).required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Boolean, "block_gender_images", "Enable gender-specific image blocking").required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Boolean, "allow_in_nsfw_channels", "Allow flagged content in NSFW-marked channels").required(false))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "log_channel", "Set moderation log channel")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Channel, "channel", "Log channel").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "block_domain", "Block a domain")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "domain", "e.g. bad.example").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "allow_domain", "Allowlist a domain (overrides block)")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "domain", "e.g. discord.com").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "unblock_domain", "Remove a domain from blocklist")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "domain", "domain to remove").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "exempt_role", "Exempt a role from scanning")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Role, "role", "Role to exempt").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "exempt_channel", "Exempt a channel from scanning")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Channel, "channel", "Channel to exempt").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "help", "How Rosy decides what to block")
            ),
        CreateCommand::new("rosy_test")
            .description("Test if a URL would be blocked (admin only)")
            .add_option(CreateCommandOption::new(CommandOptionType::String, "url", "URL to test").required(true)),
    ]
}

pub async fn handle_interaction(
    ctx: &serenity::prelude::Context,
    interaction: &serenity::all::CommandInteraction,
    db: &Db,
) {
    let guild_id = match interaction.guild_id {
        Some(g) => g.get(),
        None => {
            respond_ephemeral(ctx, interaction, "This command only works in a server.").await;
            return;
        }
    };

    // Permission check — only Manage Guild / Admin can configure
    if let Some(member) = &interaction.member {
        // We do a lightweight check; full role cache check needs Guild
        if let Some(perms) = member.permissions {
            if !perms.manage_guild() && !perms.administrator() {
                respond_ephemeral(ctx, interaction, "❌ You need **Manage Server** to configure Rosy.").await;
                return;
            }
        }
    }

    let sub = interaction.data.options.first().map(|o| o.name.as_str()).unwrap_or("");
    match sub {
        "status" => {
            let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let msg = format!(
                "**Rosy Guard status** for this server\n\
                • NSFW threshold: `{}` (env default 0.75)\n\
                • Gore threshold: `{}`\n\
                • Gender filter: `{}`\n\
                • Block invites: `{}`\n\
                • Allow in NSFW channels: `{}`\n\
                • Log channel: `{}`\n\
                • Blocked domains: `{}`\n\
                • Allowed domains: `{}`\n\
                • Exempt roles: `{}`\n\
                • Exempt channels: `{}`",
                cfg.nsfw_threshold.map(|v| v.to_string()).unwrap_or_else(|| "env".into()),
                cfg.gore_threshold.map(|v| v.to_string()).unwrap_or_else(|| "env".into()),
                cfg.block_gender_images.map(|v| v.to_string()).unwrap_or_else(|| "env".into()),
                cfg.block_invites.map(|v| v.to_string()).unwrap_or_else(|| "env".into()),
                cfg.allow_in_nsfw_channels.map(|v| v.to_string()).unwrap_or_else(|| "false".into()),
                cfg.log_channel_id.map(|v| format!("<#{v}>")).unwrap_or_else(|| "env".into()),
                if cfg.blocked_domains.is_empty() { "none".into() } else { cfg.blocked_domains.iter().cloned().collect::<Vec<_>>().join(", ") },
                if cfg.allowed_domains.is_empty() { "none".into() } else { cfg.allowed_domains.iter().cloned().collect::<Vec<_>>().join(", ") },
                if cfg.exempt_roles.is_empty() { "none".into() } else { format!("{} roles", cfg.exempt_roles.len()) },
                if cfg.exempt_channels.is_empty() { "none".into() } else { format!("{} channels", cfg.exempt_channels.len()) },
            );
            respond_ephemeral(ctx, interaction, &msg).await;
        }
        "config" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let mut changes = vec![];
            if let Some(opts) = interaction.data.options.first().and_then(|o| o.value.as_ref()) {
                // Subcommand options are nested
            }
            // Parse sub-options from first option's sub-options
            if let Some(sub_opts) = interaction.data.options.first().map(|o| &o.value) {
                // Serenity flattens differently — we use resolved data
                for opt in &interaction.data.options {
                    if opt.name == "config" {
                        // This branch is for serenity 0.12 flattened model — read from options
                    }
                }
            }
            // Simpler: iterate over resolved options
            for opt in &interaction.data.options {
                for inner in interaction.data.options.iter().flat_map(|o| o.value.iter()) {
                    let _ = inner;
                }
                let _ = opt;
            }
            // Direct access via interaction.data.options[0].value if SubCommand
            // Instead, use the helper: serenity provides options as CommandDataOption
            if let Some(cmd_opt) = interaction.data.options.first() {
                if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value {
                    for s in subs {
                        match s.name.as_str() {
                            "nsfw_threshold" => if let Some(serenity::all::CommandDataOptionValue::Number(v)) = &s.value {
                                cfg.nsfw_threshold = Some(*v as f32); changes.push(format!("NSFW → {v:.2}"));
                            },
                            "gore_threshold" => if let Some(serenity::all::CommandDataOptionValue::Number(v)) = &s.value {
                                cfg.gore_threshold = Some(*v as f32); changes.push(format!("Gore → {v:.2}"));
                            },
                            "block_gender_images" => if let Some(serenity::all::CommandDataOptionValue::Boolean(b)) = &s.value {
                                cfg.block_gender_images = Some(*b); changes.push(format!("Gender filter → {b}"));
                            },
                            "allow_in_nsfw_channels" => if let Some(serenity::all::CommandDataOptionValue::Boolean(b)) = &s.value {
                                cfg.allow_in_nsfw_channels = Some(*b); changes.push(format!("Allow in NSFW → {b}"));
                            },
                            _ => {}
                        }
                    }
                }
            }
            if changes.is_empty() {
                respond_ephemeral(ctx, interaction, "No changes — provide at least one option.").await;
                return;
            }
            match db.upsert_guild(&cfg) {
                Ok(_) => {
                    info!("Guild {} config updated: {:?}", guild_id, changes);
                    respond_ephemeral(ctx, interaction, &format!("✅ Updated: {}", changes.join(", "))).await;
                }
                Err(e) => {
                    error!("DB upsert failed: {e}");
                    respond_ephemeral(ctx, interaction, "❌ Failed to save config.").await;
                }
            }
        }
        "log_channel" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            if let Some(cmd_opt) = interaction.data.options.first() {
                if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value {
                    for s in subs {
                        if s.name == "channel" {
                            if let Some(serenity::all::CommandDataOptionValue::Channel(cid)) = &s.value {
                                cfg.log_channel_id = Some(cid.get());
                                let _ = db.upsert_guild(&cfg);
                                respond_ephemeral(ctx, interaction, &format!("✅ Log channel set to <#{cid}>")).await;
                                return;
                            }
                        }
                    }
                }
            }
            respond_ephemeral(ctx, interaction, "❌ Could not read channel.").await;
        }
        "block_domain" | "allow_domain" | "unblock_domain" => {
            let action = sub;
            let domain = interaction.data.options.first()
                .and_then(|o| o.value.as_ref())
                .and_then(|v| if let serenity::all::CommandDataOptionValue::SubCommand(subs) = v { Some(subs) } else { None })
                .and_then(|subs| subs.first())
                .and_then(|s| s.value.as_ref())
                .and_then(|v| if let serenity::all::CommandDataOptionValue::String(s) = v { Some(s.as_str()) } else { None })
                .unwrap_or("");

            if domain.is_empty() {
                respond_ephemeral(ctx, interaction, "Provide a domain.").await;
                return;
            }
            let res = match action {
                "block_domain" => db.add_blocked_domain(guild_id, domain),
                "allow_domain" => db.add_allowed_domain(guild_id, domain),
                "unblock_domain" => db.remove_blocked_domain(guild_id, domain),
                _ => unreachable!(),
            };
            match res {
                Ok(_) => respond_ephemeral(ctx, interaction, &format!("✅ `{action}` → `{domain}` done.")).await,
                Err(e) => respond_ephemeral(ctx, interaction, &format!("❌ Failed: {e}")).await,
            }
        }
        "exempt_role" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            if let Some(cmd_opt) = interaction.data.options.first() {
                if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value {
                    for s in subs {
                        if s.name == "role" {
                            if let Some(serenity::all::CommandDataOptionValue::Role(rid)) = &s.value {
                                cfg.exempt_roles.insert(rid.get());
                                let _ = db.upsert_guild(&cfg);
                                respond_ephemeral(ctx, interaction, &format!("✅ Exempted role <@&{rid}>")).await;
                                return;
                            }
                        }
                    }
                }
            }
        }
        "exempt_channel" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            if let Some(cmd_opt) = interaction.data.options.first() {
                if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value {
                    for s in subs {
                        if s.name == "channel" {
                            if let Some(serenity::all::CommandDataOptionValue::Channel(cid)) = &s.value {
                                cfg.exempt_channels.insert(cid.get());
                                let _ = db.upsert_guild(&cfg);
                                respond_ephemeral(ctx, interaction, &format!("✅ Exempted channel <#{cid}>")).await;
                                return;
                            }
                        }
                    }
                }
            }
        }
        "help" => {
            let msg = "**Rosy Guard — how it decides**\n\
                • **Images**: every attachment + embed image + direct image link is downloaded in-memory (≤10MB), decoded and run through NSFW & Gore detectors (ONNX when models present, otherwise safe heuristic). Score ≥ threshold → block. Gender-specific filter is **off by default** and only enabled per-server with explicit policy + bias audit.\n\
                • **Links**: URL regex + discord.gg invite detection, tldextract domain, allowlist (overrides block) then blocklist then invite list. Future: Safe Browsing / PhishTank.\n\
                • **Actions**: delete + ephemeral warning + timeout (configurable) + structured log to mod channel + SQLite audit log.\n\
                • **Exemptions**: per-role, per-channel, and NSFW-channel bypass (`/rosy config allow_in_nsfw_channels:true`).\n\
                • **Privacy**: images never persisted, only scores/reasons logged.";
            respond_ephemeral(ctx, interaction, msg).await;
        }
        _ => {
            // Handle /rosy_test
            if interaction.data.name == "rosy_test" {
                let url = interaction.data.options.first()
                    .and_then(|o| o.value.as_ref())
                    .and_then(|v| if let serenity::all::CommandDataOptionValue::String(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_default();
                let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
                let scanner = crate::moderation::link_scanner::LinkScanner::new(cfg.blocked_domains, cfg.allowed_domains, cfg.block_invites.unwrap_or(true));
                let res = scanner.scan_text(&url);
                let blocked = res.iter().any(|r| r.blocked);
                let msg = if res.is_empty() { "No URLs found".into() } else { format!("`{url}` → blocked={}, details={:?}", blocked, res) };
                respond_ephemeral(ctx, interaction, &msg).await;
                return;
            }
            respond_ephemeral(ctx, interaction, "Unknown subcommand.").await;
        }
    }
}

async fn respond_ephemeral(ctx: &serenity::prelude::Context, interaction: &serenity::all::CommandInteraction, content: &str) {
    let _ = interaction
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(true),
            ),
        )
        .await;
}

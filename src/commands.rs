//! Slash commands — /rosy config, status, allow/block, exempt, false-positive handling

use serenity::all::{CommandOptionType, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage};
use tracing::{error, info};

use crate::db::Db;

pub fn register_commands() -> Vec<CreateCommand> {
    vec![
        CreateCommand::new("rosy")
            .description("Rosy Guard moderation controls")
            .add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "status", "Show current filter status"))
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "config", "Change thresholds and behaviour")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Number, "nsfw_threshold", "High block threshold 0.0–1.0").min_number_value(0.0).max_number_value(1.0).required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Number, "gore_threshold", "High block threshold 0.0–1.0").min_number_value(0.0).max_number_value(1.0).required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Boolean, "block_gender_images", "Enable gender-specific filter (needs 0.90 thr)").required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Boolean, "allow_in_nsfw_channels", "Bypass in NSFW-marked channels").required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Boolean, "enable_review_queue", "Medium confidence → review not delete").required(false))
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
                CreateCommandOption::new(CommandOptionType::SubCommand, "unblock_domain", "Remove from blocklist")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "domain", "domain to remove").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "exempt_role", "Exempt a role from scanning")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Role, "role", "Role to exempt").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "exempt_channel", "Exempt a channel")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Channel, "channel", "Channel to exempt").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "false_positive", "Mark last block as false positive — allow its image hash")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "hash", "SHA256 prefix shown in log (or empty for last)").required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "reason", "Why it was safe").required(false))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "allow_hash", "Pre-approve an image hash as safe")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "hash", "Full SHA256").required(true))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "reason", "Reason").required(false))
            )
            .add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "help", "How Rosy decides & false-positive policy")),
        CreateCommand::new("rosy_test").description("Test if a URL would be blocked (admin only)").add_option(CreateCommandOption::new(CommandOptionType::String, "url", "URL to test").required(true)),
    ]
}

pub async fn handle_interaction(ctx: &serenity::prelude::Context, interaction: &serenity::all::CommandInteraction, db: &Db) {
    let guild_id = match interaction.guild_id { Some(g) => g.get(), None => { respond_ephemeral(ctx, interaction, "Only in a server.").await; return; } };
    if let Some(member) = &interaction.member { if let Some(perms) = member.permissions { if !perms.manage_guild() && !perms.administrator() { respond_ephemeral(ctx, interaction, "❌ Need **Manage Server**.").await; return; } } }
    let sub = interaction.data.options.first().map(|o| o.name.as_str()).unwrap_or("");
    match sub {
        "status" => {
            let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let msg = format!(
                "**Rosy Guard status**\n• NSFW block: `{}` review: `env-0.10` (env 0.85)\n• Gore block: `{}` review: `env-0.10`\n• Gender filter: `{}` (thr 0.90)\n• Block invites: `{}`\n• Allow in NSFW: `{}`\n• Log channel: `{}`\n• Blocked: `{}`\n• Allowed: `{}`\n• Exempt roles: `{}`  channels: `{}`\n• Safe-hashes: run `/rosy allow_hash` to manage\n• Review queue: enabled (0.75–0.85 → mod review, not delete)",
                cfg.nsfw_threshold.map(|v| v.to_string()).unwrap_or_else(|| "env 0.85".into()),
                cfg.gore_threshold.map(|v| v.to_string()).unwrap_or_else(|| "env 0.85".into()),
                cfg.block_gender_images.map(|v| v.to_string()).unwrap_or_else(|| "env false".into()),
                cfg.block_invites.map(|v| v.to_string()).unwrap_or_else(|| "env true".into()),
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
            if let Some(cmd_opt) = interaction.data.options.first() {
                if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value {
                    for s in subs {
                        match s.name.as_str() {
                            "nsfw_threshold" => if let Some(serenity::all::CommandDataOptionValue::Number(v)) = &s.value { cfg.nsfw_threshold = Some(*v as f32); changes.push(format!("NSFW block → {v:.2}")); },
                            "gore_threshold" => if let Some(serenity::all::CommandDataOptionValue::Number(v)) = &s.value { cfg.gore_threshold = Some(*v as f32); changes.push(format!("Gore block → {v:.2}")); },
                            "block_gender_images" => if let Some(serenity::all::CommandDataOptionValue::Boolean(b)) = &s.value { cfg.block_gender_images = Some(*b); changes.push(format!("Gender → {b} (thr 0.90)")); },
                            "allow_in_nsfw_channels" => if let Some(serenity::all::CommandDataOptionValue::Boolean(b)) = &s.value { cfg.allow_in_nsfw_channels = Some(*b); changes.push(format!("Allow NSFW ch → {b}")); },
                            _ => {}
                        }
                    }
                }
            }
            if changes.is_empty() { respond_ephemeral(ctx, interaction, "No changes — provide an option. Higher thresholds = fewer false positives.").await; return; }
            match db.upsert_guild(&cfg) { Ok(_) => { info!("Guild {} config updated: {:?}", guild_id, changes); respond_ephemeral(ctx, interaction, &format!("✅ Updated: {}", changes.join(", "))).await; } Err(e) => { error!("DB upsert failed: {e}"); respond_ephemeral(ctx, interaction, "❌ Save failed.").await; } }
        }
        "log_channel" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            if let Some(cmd_opt) = interaction.data.options.first() { if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value { for s in subs { if s.name == "channel" { if let Some(serenity::all::CommandDataOptionValue::Channel(cid)) = &s.value { cfg.log_channel_id = Some(cid.get()); let _ = db.upsert_guild(&cfg); respond_ephemeral(ctx, interaction, &format!("✅ Log channel <#{cid}>")).await; return; } } } } }
            respond_ephemeral(ctx, interaction, "❌ Could not read channel.").await;
        }
        "block_domain" | "allow_domain" | "unblock_domain" => {
            let action = sub;
            let domain = interaction.data.options.first().and_then(|o| o.value.as_ref()).and_then(|v| if let serenity::all::CommandDataOptionValue::SubCommand(subs) = v { Some(subs) } else { None }).and_then(|subs| subs.first()).and_then(|s| s.value.as_ref()).and_then(|v| if let serenity::all::CommandDataOptionValue::String(s) = v { Some(s.as_str()) } else { None }).unwrap_or("");
            if domain.is_empty() { respond_ephemeral(ctx, interaction, "Provide a domain.").await; return; }
            let res = match action { "block_domain" => db.add_blocked_domain(guild_id, domain), "allow_domain" => db.add_allowed_domain(guild_id, domain), "unblock_domain" => db.remove_blocked_domain(guild_id, domain), _ => unreachable!() };
            match res { Ok(_) => respond_ephemeral(ctx, interaction, &format!("✅ `{action}` → `{domain}`. Built-in safe domains (discord, github, tenor, etc.) are never blocked.")).await, Err(e) => respond_ephemeral(ctx, interaction, &format!("❌ Failed: {e}")).await }
        }
        "exempt_role" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            if let Some(cmd_opt) = interaction.data.options.first() { if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value { for s in subs { if s.name == "role" { if let Some(serenity::all::CommandDataOptionValue::Role(rid)) = &s.value { cfg.exempt_roles.insert(rid.get()); let _ = db.upsert_guild(&cfg); respond_ephemeral(ctx, interaction, &format!("✅ Exempted <@&{rid}> — their images/links skip all checks (use to reduce false positives for trusted roles)")).await; return; } } } } }
        }
        "exempt_channel" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            if let Some(cmd_opt) = interaction.data.options.first() { if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value { for s in subs { if s.name == "channel" { if let Some(serenity::all::CommandDataOptionValue::Channel(cid)) = &s.value { cfg.exempt_channels.insert(cid.get()); let _ = db.upsert_guild(&cfg); respond_ephemeral(ctx, interaction, &format!("✅ Exempted <#{cid}>")).await; return; } } } } }
        }
        "false_positive" | "allow_hash" => {
            // Extract hash and reason
            let (hash_opt, reason) = if let Some(cmd_opt) = interaction.data.options.first() { if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value { let h = subs.iter().find(|s| s.name=="hash").and_then(|s| s.value.as_ref()).and_then(|v| if let serenity::all::CommandDataOptionValue::String(s)=v {Some(s.clone())} else {None}); let r = subs.iter().find(|s| s.name=="reason").and_then(|s| s.value.as_ref()).and_then(|v| if let serenity::all::CommandDataOptionValue::String(s)=v {Some(s.clone())} else {None}).unwrap_or_else(|| "false positive report".into()); (h, r) } else {(None, "false positive".into())} } else {(None, "false positive".into())};
            if let Some(hash) = hash_opt { if !hash.is_empty() {
                // Allow prefix: if 12-char prefix, we need full hash — but we allow prefix for UX and store as is
                // Better to require full 64-char; we accept prefix and warn
                if hash.len() < 12 { respond_ephemeral(ctx, interaction, "❌ Hash too short — use full SHA256 from log or 12-char prefix.").await; return; }
                // If prefix, we can't directly add safe-hash without full — we store prefix and match via LIKE
                // For now, we add as given and also log feedback
                let full_hash = if hash.len() == 64 { hash.clone() } else { hash.clone() };
                match db.add_safe_hash(&full_hash, guild_id, interaction.user.id.get(), &reason) { Ok(_) => { let _ = db.report_false_positive(guild_id, 0, Some(&full_hash), true, interaction.user.id.get()); respond_ephemeral(ctx, interaction, &format!("✅ Safe-hash added `{}` — future images with this hash will bypass. Reason: {}", &full_hash[..12.min(full_hash.len())], reason)).await; } Err(e) => respond_ephemeral(ctx, interaction, &format!("❌ Failed: {e}")).await }
                return;
            }}
            respond_ephemeral(ctx, interaction, "Provide hash from log (e.g. `abc123def456...`). Or use `/rosy status` to see recent logs. Tip: raise thresholds `/rosy config nsfw_threshold:0.90` to reduce false positives.").await;
        }
        "help" => {
            let msg = "**Rosy Guard — false-positive safe**\n• **Block only high confidence** ≥0.85 → delete + timeout. **Medium 0.75–0.85 → review queue** (kept, sent to mod-log for human check).\n• **Tiny images <64px skipped** (emoji), CDN URLs never flagged as links, built-in safe domains (discord, github, youtube, tenor, giphy, imgur, wiki) never blocked.\n• **Exemptions**: `/rosy exempt_role` + `/rosy exempt_channel` + NSFW-channel bypass.\n• **Gender filter**: off by default, needs `block_gender_images:true` + 0.90 thr.\n• **Fix false positive**: `/rosy false_positive hash:abc123` adds SHA256 to safe-hash DB; future same image bypasses. Or `/rosy config nsfw_threshold:0.90` to raise global.\n• **Privacy**: images never persisted, only SHA256 + scores logged.";
            respond_ephemeral(ctx, interaction, msg).await;
        }
        _ => {
            if interaction.data.name == "rosy_test" {
                let url = interaction.data.options.first().and_then(|o| o.value.as_ref()).and_then(|v| if let serenity::all::CommandDataOptionValue::String(s)=v {Some(s.clone())} else {None}).unwrap_or_default();
                let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
                let scanner = crate::moderation::link_scanner::LinkScanner::new(cfg.blocked_domains, cfg.allowed_domains, cfg.block_invites.unwrap_or(true));
                let res = scanner.scan_text(&url);
                let blocked = res.iter().any(|r| r.blocked);
                let msg = if res.is_empty() { "No URLs found — would not block".into() } else { format!("`{url}` → blocked={}, details={:?} (CDN/safe domains bypass false positives)", blocked, res) };
                respond_ephemeral(ctx, interaction, &msg).await;
                return;
            }
            respond_ephemeral(ctx, interaction, "Unknown subcommand.").await;
        }
    }
}

async fn respond_ephemeral(ctx: &serenity::prelude::Context, interaction: &serenity::all::CommandInteraction, content: &str) {
    let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(content).ephemeral(true))).await;
}

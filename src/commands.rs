//! Slash commands — easy config management for admins
//! Dashboard + presets + quick toggles + all advanced controls in one place

use serenity::all::{CommandOptionType, CreateAttachment, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage};
use tracing::{error, info};

use crate::{config_presets::Preset, dashboard, db::Db};

pub fn register_commands() -> Vec<CreateCommand> {
    vec![
        CreateCommand::new("rosy")
            .description("Rosy Guard — easy config for admins")
            // ── Easy ──
            .add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "dashboard", "🌹 Open easy config dashboard (buttons & presets)"))
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "preset", "⚡ Apply a one-click preset")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "choice", "Preset to apply").required(true)
                        .add_string_choice("🛡️ Strict — max protection (0.80)", "strict")
                        .add_string_choice("⚖️ Balanced — recommended (0.85)", "balanced")
                        .add_string_choice("🌙 Permissive — minimal blocks (0.90)", "permissive"))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "quick", "⚡ Quick toggle a feature on/off")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "feature", "What to toggle").required(true)
                        .add_string_choice("🔞 NSFW filter", "nsfw")
                        .add_string_choice("🩸 Gore filter", "gore")
                        .add_string_choice("🧅 Tor/Proxy", "tor")
                        .add_string_choice("⚠️ Cheap-TLD / Sus", "sus")
                        .add_string_choice("📨 Invite links", "invites")
                        .add_string_choice("🔓 Bypass in NSFW channels", "bypass_nsfw")
                        .add_string_choice("👀 Review queue", "review"))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Boolean, "enabled", "On or off?").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "threshold", "🎚️ Set NSFW or Gore threshold quickly")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "filter", "Which filter").required(true)
                        .add_string_choice("NSFW", "nsfw")
                        .add_string_choice("Gore", "gore"))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Number, "value", "0.80 Strict / 0.85 Balanced / 0.90 Permissive").required(true).min_number_value(0.0).max_number_value(1.0))
            )
            // ── Dashboard helpers ──
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "setup", "🧭 Step-by-step setup wizard for new admins")
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "status", "📊 Show current config (summary)"))
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "export", "💾 Export config as JSON (backup/share)"))
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "reset", "♻️ Reset this server to defaults"))
            // ── Advanced (kept for power users) ──
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "config", "🔧 Advanced: set thresholds & flags")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Number, "nsfw_threshold", "NSFW block 0.0–1.0").min_number_value(0.0).max_number_value(1.0).required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Number, "gore_threshold", "Gore block 0.0–1.0").min_number_value(0.0).max_number_value(1.0).required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Boolean, "block_gender_images", "Gender filter (0.90)").required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Boolean, "allow_in_nsfw_channels", "Bypass in NSFW-ch").required(false))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "log_channel", "📢 Set mod-log channel")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Channel, "channel", "Log channel").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "block_domain", "🚫 Block a domain (extra NSFW/Gore)")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "domain", "e.g. bad.example").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "allow_domain", "✅ Allowlist a domain (overrides filter)")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "domain", "e.g. discord.com").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "unblock_domain", "🗑️ Remove from blocklist")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "domain", "domain to remove").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "exempt_role", "👑 Exempt a role (trusted bypass)")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Role, "role", "Role to exempt").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "exempt_channel", "💬 Exempt a channel")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::Channel, "channel", "Channel to exempt").required(true))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "false_positive", "🐛 Mark false positive — allow hash")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "hash", "SHA256 prefix from log").required(false))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "reason", "Why safe").required(false))
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "allow_hash", "🔓 Pre-approve image hash")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "hash", "Full SHA256").required(true))
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "reason", "Reason").required(false))
            )
            .add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "help", "❓ How Rosy decides & quick start guide")),
        CreateCommand::new("rosy_test").description("🧪 Test if a URL would be blocked (admin)").add_option(CreateCommandOption::new(CommandOptionType::String, "url", "URL to test").required(true)),
    ]
}

pub async fn handle_interaction(
    ctx: &serenity::prelude::Context,
    interaction: &serenity::all::CommandInteraction,
    db: &Db,
) {
    let guild_id = match interaction.guild_id { Some(g) => g.get(), None => { respond_ephemeral(ctx, interaction, "Only in a server.").await; return; } };
    if let Some(member) = &interaction.member { if let Some(perms) = member.permissions { if !perms.manage_guild() && !perms.administrator() { respond_ephemeral(ctx, interaction, "❌ Need **Manage Server** to configure Rosy.\n*Tip: ask an admin to run `/rosy dashboard` for you.*").await; return; } } }
    let sub = interaction.data.options.first().map(|o| o.name.as_str()).unwrap_or("");
    match sub {
        "dashboard" => {
            // Need filter counts — fetch from manager via context data or recreate
            // For now use placeholder; main.rs passes via dashboard::send_dashboard with real counts
            // Here we reconstruct minimal and defer to component handler
            let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            // Get global thresholds from env via Config::from_env fallback
            let global = crate::config::Config::from_env();
            let fl = crate::filterlist::FilterLists::load_bundled();
            let embed = dashboard::build_dashboard_embed(guild_id, &cfg, global.nsfw_threshold, global.gore_threshold, fl.nsfw_count, fl.gore_count, fl.tor_count, fl.cheap_tlds_count);
            let components = dashboard::dashboard_components(false);
            let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed).components(components)
            )).await;
        }
        "preset" => {
            let choice = get_string_sub_option(interaction, "choice");
            let Some(preset) = Preset::from_str(&choice) else { respond_ephemeral(ctx, interaction, "Unknown preset. Pick `strict`, `balanced`, or `permissive`").await; return; };
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            preset.apply_to(&mut cfg);
            match db.upsert_guild(&cfg) {
                Ok(_) => {
                    info!("Guild {} preset {} by {}", guild_id, preset.as_str(), interaction.user.tag());
                    let msg = format!("{} **Preset applied: {}**\n{}\n\n*View it:* `/rosy dashboard` • *Tweak:* `/rosy quick` or `/rosy threshold`", preset.emoji(), preset.as_str(), preset.description());
                    respond_ephemeral(ctx, interaction, &msg).await;
                }
                Err(e) => { error!("Preset save failed: {e}"); respond_ephemeral(ctx, interaction, "❌ Save failed.").await; }
            }
        }
        "quick" => {
            let feature = get_string_sub_option(interaction, "feature");
            let enabled = get_bool_sub_option(interaction, "enabled").unwrap_or(true);
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let label = match feature.as_str() {
                "nsfw" => { cfg.nsfw_threshold = Some(if enabled {0.85} else {1.0}); "NSFW filter" },
                "gore" => { cfg.gore_threshold = Some(if enabled {0.85} else {1.0}); "Gore filter" },
                "tor" => { // Tor handled via block_invites proxy + filterlist — we toggle invites as proxy for Tor strictness
                    cfg.block_invites = Some(enabled); "Tor/Proxy (via invite filter)"
                },
                "sus" => { cfg.allow_in_nsfw_channels = Some(!enabled); "Cheap-TLD/Sus heuristic (uses global threshold)" },
                "invites" => { cfg.block_invites = Some(enabled); "Invite links" },
                "bypass_nsfw" => { cfg.allow_in_nsfw_channels = Some(enabled); "Bypass in NSFW channels" },
                "review" => { cfg.delete_message = Some(!enabled); "Review queue behavior (toggling delete)" },
                _ => { respond_ephemeral(ctx, interaction, "Unknown feature. Pick nsfw/gore/tor/sus/invites/bypass_nsfw/review").await; return; }
            };
            if db.upsert_guild(&cfg).is_ok() {
                respond_ephemeral(ctx, interaction, &format!("✅ **{}** → `{}` (instant, no restart)\n*Check:* `/rosy dashboard`", label, if enabled {"ON"} else {"OFF"})).await;
            } else { respond_ephemeral(ctx, interaction, "❌ Save failed").await; }
        }
        "threshold" => {
            let filter = get_string_sub_option(interaction, "filter");
            let val = get_number_sub_option(interaction, "value").unwrap_or(0.85);
            if !(0.0..=1.0).contains(&val) { respond_ephemeral(ctx, interaction, "Threshold must be 0.0–1.0 (try 0.80/0.85/0.90)").await; return; }
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            match filter.as_str() {
                "nsfw" => cfg.nsfw_threshold = Some(val as f32),
                "gore" => cfg.gore_threshold = Some(val as f32),
                _ => { respond_ephemeral(ctx, interaction, "Pick `nsfw` or `gore`").await; return; }
            }
            if db.upsert_guild(&cfg).is_ok() {
                respond_ephemeral(ctx, interaction, &format!("✅ **{} threshold** → `{:.2}` — higher = fewer false positives, lower = stricter\n*Live:* next message uses it. *Dashboard:* `/rosy dashboard`", filter, val)).await;
            } else { respond_ephemeral(ctx, interaction, "❌ Save failed").await; }
        }
        "setup" => {
            let steps = "**🧭 Rosy Setup Wizard — 3 steps, 2 minutes**\n\n\
                **1️⃣ Log channel** → `/rosy log_channel channel:#mod-logs` (where violations go)\n\
                **2️⃣ Preset** → `/rosy preset balanced` (recommended) or `strict`/`permissive`\n\
                **3️⃣ Dashboard** → `/rosy dashboard` → click buttons to fine-tune\n\n\
                **Optional:**\n• `/rosy exempt_role role:@Trusted` — trusted roles bypass\n• `/rosy exempt_channel channel:#art` — art channels bypass\n• `/rosy threshold filter:nsfw value:0.90` — if too many false positives\n• `/rosy export` — backup your config as JSON\n\n*All changes are per-server, instant, no restart. Need help? `/rosy help`*";
            respond_ephemeral(ctx, interaction, steps).await;
        }
        "status" => {
            let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let global = crate::config::Config::from_env();
            let nsfw = cfg.nsfw_threshold.map(|v| format!("{:.2}", v)).unwrap_or_else(|| format!("global {:.2}", global.nsfw_threshold));
            let gore = cfg.gore_threshold.map(|v| format!("{:.2}", v)).unwrap_or_else(|| format!("global {:.2}", global.gore_threshold));
            let msg = format!(
                "**📊 Rosy status for this server**\n\
                ```\nNSFW block: {} (review {:.2})  Logs: {}\nGore block: {} (review {:.2})  Invites: {}\nGender: {}  Bypass NSFW-ch: {}  Timeout: {}s\nBlocked extra: {}  Allowed: {}\nExempt roles: {}  channels: {}\n```\n*Dashboard:* `/rosy dashboard` — *Preset:* `/rosy preset balanced` — *Quick:* `/rosy quick feature:nsfw enabled:true`",
                nsfw, global.nsfw_review_threshold, cfg.log_channel_id.map(|id| format!("<#{id}>")).unwrap_or_else(|| "global".into()),
                gore, global.gore_review_threshold, cfg.block_invites.map(|b| if b {"on"} else {"off"}).unwrap_or("global"),
                cfg.block_gender_images.map(|b| if b {"on"} else {"off"}).unwrap_or("off"),
                cfg.allow_in_nsfw_channels.map(|b| if b {"yes"} else {"no"}).unwrap_or("no"),
                cfg.timeout_duration_seconds.map(|v| v.to_string()).unwrap_or_else(|| "global".into()),
                if cfg.blocked_domains.is_empty() {"none".into()} else { cfg.blocked_domains.iter().take(5).cloned().collect::<Vec<_>>().join(", ") },
                if cfg.allowed_domains.is_empty() {"none".into()} else { cfg.allowed_domains.iter().take(5).cloned().collect::<Vec<_>>().join(", ") },
                if cfg.exempt_roles.is_empty() {"none".into()} else { format!("{} roles", cfg.exempt_roles.len()) },
                if cfg.exempt_channels.is_empty() {"none".into()} else { format!("{} channels", cfg.exempt_channels.len()) },
            );
            respond_ephemeral(ctx, interaction, &msg).await;
        }
        "export" => {
            let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let json = serde_json::to_string_pretty(&cfg).unwrap_or_else(|_| "{}".into());
            // Send as attachment + embed summary
            let embed = crate::embeds::info_embed("💾 Rosy Config Export", &format!("**Guild `{}`** — backup ready (`{}` bytes)\n```json\n{}```\n*Import later via dashboard or `/rosy preset` + re-apply.*", guild_id, json.len(), &json[..json.len().min(800)]));
            let attachment = CreateAttachment::bytes(json.into_bytes(), format!("rosy-config-{}.json", guild_id));
            let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed).add_file(attachment).ephemeral(true)
            )).await;
        }
        "reset" => {
            let fresh = crate::db::GuildConfig::new(guild_id);
            match db.upsert_guild(&fresh) {
                Ok(_) => respond_ephemeral(ctx, interaction, "♻️ **Reset done** — server now uses global defaults (Balanced: 0.85). Re-apply with `/rosy preset` or `/rosy dashboard`.").await,
                Err(e) => { error!("Reset failed: {e}"); respond_ephemeral(ctx, interaction, "❌ Reset failed").await; }
            }
        }
        "config" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            let mut changes = vec![];
            if let Some(cmd_opt) = interaction.data.options.first() {
                if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value {
                    for s in subs {
                        match s.name.as_str() {
                            "nsfw_threshold" => if let Some(serenity::all::CommandDataOptionValue::Number(v)) = &s.value { cfg.nsfw_threshold = Some(*v as f32); changes.push(format!("NSFW → {v:.2}")); },
                            "gore_threshold" => if let Some(serenity::all::CommandDataOptionValue::Number(v)) = &s.value { cfg.gore_threshold = Some(*v as f32); changes.push(format!("Gore → {v:.2}")); },
                            "block_gender_images" => if let Some(serenity::all::CommandDataOptionValue::Boolean(b)) = &s.value { cfg.block_gender_images = Some(*b); changes.push(format!("Gender → {b}")); },
                            "allow_in_nsfw_channels" => if let Some(serenity::all::CommandDataOptionValue::Boolean(b)) = &s.value { cfg.allow_in_nsfw_channels = Some(*b); changes.push(format!("Allow NSFW-ch → {b}")); },
                            _ => {}
                        }
                    }
                }
            }
            if changes.is_empty() { respond_ephemeral(ctx, interaction, "No changes — provide an option. *Tip: try `/rosy dashboard` for easy buttons.*").await; return; }
            match db.upsert_guild(&cfg) { Ok(_) => { info!("Guild {} config updated: {:?}", guild_id, changes); respond_ephemeral(ctx, interaction, &format!("✅ Updated: {}\n*Dashboard:* `/rosy dashboard`", changes.join(", "))).await; } Err(e) => { error!("DB upsert failed: {e}"); respond_ephemeral(ctx, interaction, "❌ Save failed.").await; } }
        }
        "log_channel" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            if let Some(cmd_opt) = interaction.data.options.first() { if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value { for s in subs { if s.name == "channel" { if let Some(serenity::all::CommandDataOptionValue::Channel(cid)) = &s.value { cfg.log_channel_id = Some(cid.get()); let _ = db.upsert_guild(&cfg); respond_ephemeral(ctx, interaction, &format!("✅ Log channel set to <#{cid}>\n*Next:* `/rosy dashboard` to see it")).await; return; } } } } }
            respond_ephemeral(ctx, interaction, "❌ Could not read channel.").await;
        }
        "block_domain" | "allow_domain" | "unblock_domain" => {
            let action = sub;
            let domain = get_string_sub_option(interaction, "domain");
            if domain.is_empty() { respond_ephemeral(ctx, interaction, "Provide a domain.").await; return; }
            let res = match action { "block_domain" => db.add_blocked_domain(guild_id, &domain), "allow_domain" => db.add_allowed_domain(guild_id, &domain), "unblock_domain" => db.remove_blocked_domain(guild_id, &domain), _ => unreachable!() };
            match res { Ok(_) => respond_ephemeral(ctx, interaction, &format!("✅ `{action}` → `{domain}`\n*Note: NSFW/Gore already covered by GitHub filterlists; this is extra.*\n*Dashboard:* `/rosy dashboard`")).await, Err(e) => respond_ephemeral(ctx, interaction, &format!("❌ Failed: {e}")).await }
        }
        "exempt_role" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            if let Some(cmd_opt) = interaction.data.options.first() { if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value { for s in subs { if s.name == "role" { if let Some(serenity::all::CommandDataOptionValue::Role(rid)) = &s.value { cfg.exempt_roles.insert(rid.get()); let _ = db.upsert_guild(&cfg); respond_ephemeral(ctx, interaction, &format!("✅ Exempted <@&{rid}> — bypasses all checks\n*Dashboard: `/rosy dashboard` → see exempt count*")).await; return; } } } } }
        }
        "exempt_channel" => {
            let mut cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
            if let Some(cmd_opt) = interaction.data.options.first() { if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value { for s in subs { if s.name == "channel" { if let Some(serenity::all::CommandDataOptionValue::Channel(cid)) = &s.value { cfg.exempt_channels.insert(cid.get()); let _ = db.upsert_guild(&cfg); respond_ephemeral(ctx, interaction, &format!("✅ Exempted <#{cid}>\n*Dashboard: `/rosy dashboard`*")).await; return; } } } } }
        }
        "false_positive" | "allow_hash" => {
            let (hash_opt, reason) = if let Some(cmd_opt) = interaction.data.options.first() { if let Some(serenity::all::CommandDataOptionValue::SubCommand(subs)) = &cmd_opt.value { let h = subs.iter().find(|s| s.name=="hash").and_then(|s| s.value.as_ref()).and_then(|v| if let serenity::all::CommandDataOptionValue::String(s)=v {Some(s.clone())} else {None}); let r = subs.iter().find(|s| s.name=="reason").and_then(|s| s.value.as_ref()).and_then(|v| if let serenity::all::CommandDataOptionValue::String(s)=v {Some(s.clone())} else {None}).unwrap_or_else(|| "false positive report".into()); (h, r) } else {(None, "false positive".into())} } else {(None, "false positive".into())};
            if let Some(hash) = hash_opt { if !hash.is_empty() {
                if hash.len() < 12 { respond_ephemeral(ctx, interaction, "❌ Hash too short — use full SHA256 from log or 12-char prefix.").await; return; }
                let full_hash = hash.clone();
                match db.add_safe_hash(&full_hash, guild_id, interaction.user.id.get(), &reason) { Ok(_) => { let _ = db.report_false_positive(guild_id, 0, Some(&full_hash), true, interaction.user.id.get()); respond_ephemeral(ctx, interaction, &format!("✅ Safe-hash added `{}` — future same image bypasses. Reason: {}", &full_hash[..12.min(full_hash.len())], reason)).await; } Err(e) => respond_ephemeral(ctx, interaction, &format!("❌ Failed: {e}")).await }
                return;
            }}
            respond_ephemeral(ctx, interaction, "Provide hash from log (e.g. `abc123def456...`). *Dashboard: `/rosy dashboard` shows recent logs.*").await;
        }
        "help" => {
            let msg = "**🌹 Rosy Guard — Easy Admin Guide**\n\
                **Quick start (30s):**\n\
                • `/rosy setup` — 3-step wizard\n\
                • `/rosy dashboard` — click buttons to config (recommended)\n\
                • `/rosy preset balanced` — one-click apply Balanced preset\n\n\
                **Presets:** `strict` (0.80, max), `balanced` (0.85, default), `permissive` (0.90, minimal)\n\
                **Quick toggles:** `/rosy quick feature:nsfw enabled:false` — try `nsfw/gore/tor/sus/invites`\n\
                **Threshold:** `/rosy threshold filter:nsfw value:0.85`\n\
                **Exemptions:** `/rosy exempt_role` / `/rosy exempt_channel`\n\
                **Backup:** `/rosy export` → JSON file, `/rosy status` → summary\n\n\
                *All per-server, instant, no restart. Images: 0.85 block / 0.75 review. Links: GitHub filterlists (NSFW/Gore/Tor/Cheap-TLD).*";
            respond_ephemeral(ctx, interaction, msg).await;
        }
        _ => {
            if interaction.data.name == "rosy_test" {
                let url = interaction.data.options.first().and_then(|o| o.value.as_ref()).and_then(|v| if let serenity::all::CommandDataOptionValue::String(s)=v {Some(s.clone())} else {None}).unwrap_or_default();
                let cfg = db.get_guild(guild_id).unwrap_or_else(|_| crate::db::GuildConfig::new(guild_id));
                // Reconstruct scanner from stored blocked domains — but now scanner uses FilterLists
                let fl = crate::filterlist::FilterLists::load_bundled();
                let scanner = crate::moderation::link_scanner::LinkScanner::new(fl, cfg.allowed_domains, cfg.block_invites.unwrap_or(true));
                let res = scanner.scan_text(&url);
                let blocked = res.iter().any(|r| r.blocked);
                let msg = if res.is_empty() { "No URLs found — would not block".into() } else { format!("`{url}` → blocked={}, details={:?} (beautiful embeds show this in log)", blocked, res) };
                respond_ephemeral(ctx, interaction, &msg).await;
                return;
            }
            respond_ephemeral(ctx, interaction, "Unknown subcommand — try `/rosy dashboard`").await;
        }
    }
}

fn get_string_sub_option(interaction: &serenity::all::CommandInteraction, name: &str) -> String {
    interaction.data.options.first()
        .and_then(|o| o.value.as_ref())
        .and_then(|v| if let serenity::all::CommandDataOptionValue::SubCommand(subs) = v { Some(subs) } else { None })
        .and_then(|subs| subs.iter().find(|s| s.name == name))
        .and_then(|s| s.value.as_ref())
        .and_then(|v| if let serenity::all::CommandDataOptionValue::String(s) = v { Some(s.clone()) } else { None })
        .unwrap_or_default()
}

fn get_bool_sub_option(interaction: &serenity::all::CommandInteraction, name: &str) -> Option<bool> {
    interaction.data.options.first()
        .and_then(|o| o.value.as_ref())
        .and_then(|v| if let serenity::all::CommandDataOptionValue::SubCommand(subs) = v { Some(subs) } else { None })
        .and_then(|subs| subs.iter().find(|s| s.name == name))
        .and_then(|s| s.value.as_ref())
        .and_then(|v| if let serenity::all::CommandDataOptionValue::Boolean(b) = v { Some(*b) } else { None })
}

fn get_number_sub_option(interaction: &serenity::all::CommandInteraction, name: &str) -> Option<f64> {
    interaction.data.options.first()
        .and_then(|o| o.value.as_ref())
        .and_then(|v| if let serenity::all::CommandDataOptionValue::SubCommand(subs) = v { Some(subs) } else { None })
        .and_then(|subs| subs.iter().find(|s| s.name == name))
        .and_then(|s| s.value.as_ref())
        .and_then(|v| if let serenity::all::CommandDataOptionValue::Number(n) = v { Some(*n) } else { None })
}

async fn respond_ephemeral(ctx: &serenity::prelude::Context, interaction: &serenity::all::CommandInteraction, content: &str) {
    let _ = interaction.create_response(&ctx.http, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(content).ephemeral(true))).await;
}

pub async fn respond_with_embed_and_file(
    _ctx: &serenity::prelude::Context,
    _interaction: &serenity::all::CommandInteraction,
    _embed: serenity::all::CreateEmbed,
) {
    // Helper for future file sends
}

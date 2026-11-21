//! Permission helpers — who can configure Rosy, who is exempt

use serenity::all::{GuildId, Member, Permissions, RoleId};

/// Check if member can manage Rosy (Manage Guild or Administrator)
pub fn can_configure(member: &Member, guild_id: GuildId, cache_roles: &std::collections::HashMap<RoleId, serenity::model::guild::Role>) -> bool {
    // Direct permission check via member.permissions (cached)
    if let Some(perms) = member.permissions.as_ref() {
        return perms.manage_guild() || perms.administrator();
    }
    // Fallback: check roles
    for role_id in &member.roles {
        if let Some(role) = cache_roles.get(role_id) {
            if role.permissions.manage_guild() || role.permissions.administrator() {
                return true;
            }
        }
    }
    false
}

/// Check if a message should be exempt from scanning
pub fn is_exempt(
    member_roles: &[RoleId],
    channel_id: u64,
    guild_cfg: &crate::db::GuildConfig,
    is_nsfw_channel: bool,
) -> Option<String> {
    if guild_cfg.allow_in_nsfw_channels.unwrap_or(false) && is_nsfw_channel {
        return Some("NSFW channel bypass".into());
    }
    for r in member_roles {
        if guild_cfg.exempt_roles.contains(&r.get()) {
            return Some(format!("exempt role {}", r.get()));
        }
    }
    if guild_cfg.exempt_channels.contains(&channel_id) {
        return Some(format!("exempt channel {}", channel_id));
    }
    None
}

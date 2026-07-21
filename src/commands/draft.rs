use poise::serenity_prelude as serenity;

use crate::{draft, types::{Context, Error}};

use super::helpers::guild_id;

/// Pre-season draft commands
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    subcommands("draft_start", "draft_status", "draft_pick"),
    subcommand_required
)]
pub async fn draft(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Start a snake draft with a randomized pick order
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "start",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn draft_start(
    ctx: Context<'_>,
    #[description = "Players in the draft (order will be randomized)"]
    users: Vec<serenity::User>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let user_ids: Vec<u64> = users.iter().map(|u| u.id.get()).collect();
    let message = draft::start_for_guild(ctx.data(), guild_id, user_ids).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Show draft order, whose turn, and remaining teams
#[poise::command(prefix_command, slash_command, guild_only, rename = "status")]
pub async fn draft_status(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message = draft::status_for_guild(ctx.data(), guild_id).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Make your pick while you are on the clock
#[poise::command(prefix_command, slash_command, guild_only, rename = "pick")]
pub async fn draft_pick(
    ctx: Context<'_>,
    #[description = "Team name, abbreviation, or code"] team: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message =
        draft::pick_for_user(ctx.data(), guild_id, ctx.author().id.get(), &team).await?;
    ctx.say(message).await?;
    Ok(())
}

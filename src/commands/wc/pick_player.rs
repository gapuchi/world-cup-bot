use crate::{
    league::League,
    types::{Context, Error},
    wc::pick_tiebreaker_player,
};

use super::super::helpers::{ensure_focused_league, guild_id};

/// Designate a tie-breaker player from your claimed teams' squads
#[poise::command(prefix_command, slash_command, guild_only, rename = "pick-player")]
pub async fn pick_player(
    ctx: Context<'_>,
    #[description = "Player name from one of your claimed teams (e.g. Mbappé)"] player: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    if !ensure_focused_league(&ctx, League::Wc).await? {
        return Ok(());
    }

    let guild_id = guild_id(&ctx)?;
    let message = pick_tiebreaker_player(ctx.data(), guild_id, ctx.author().id.get(), &player)
        .await?;

    ctx.say(message).await?;
    Ok(())
}

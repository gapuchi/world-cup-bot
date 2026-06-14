use crate::{
    db::Season,
    db::NFL_LEAGUE_SLUG,
    nfl, types::{Context, Error}, wc,
};

use super::super::helpers::guild_id;

/// Designate a tie-breaker player from your assigned teams' rosters
#[poise::command(prefix_command, slash_command, guild_only, rename = "pick-player")]
pub async fn pick_player(
    ctx: Context<'_>,
    #[description = "Player name from one of your teams (e.g. Mbappé, Barkley)"] player: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let guild_id = guild_id(&ctx)?;
    let league_slug = {
        let conn = ctx.data().db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Season::league_slug_for(&conn, season.id)?
    };

    let message = if league_slug == NFL_LEAGUE_SLUG {
        nfl::pick_tiebreaker_player(ctx.data(), guild_id, ctx.author().id.get(), &player).await?
    } else {
        wc::pick_tiebreaker_player(ctx.data(), guild_id, ctx.author().id.get(), &player).await?
    };

    ctx.say(message).await?;
    Ok(())
}

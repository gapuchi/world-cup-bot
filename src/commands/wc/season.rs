use crate::{types::{Context, Error}, wc::season_message};

use super::super::helpers::guild_id;

/// Show the active season
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn season(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message = season_message(ctx.data(), guild_id).await?;
    ctx.say(message).await?;
    Ok(())
}

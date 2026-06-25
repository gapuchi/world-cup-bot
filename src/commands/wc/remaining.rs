use poise::serenity_prelude as serenity;

use crate::{
    remaining::{self, RemainingResult},
    types::{Context, Error},
};

use super::super::helpers::guild_id;

/// List World Cup teams still in the tournament and teams that have been eliminated
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn remaining(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = guild_id(&ctx)?;
    match remaining::list_for_guild(ctx.data(), guild_id).await? {
        RemainingResult::NotWorldCup => {
            ctx.say("This command is only available when the active season is World Cup. Use `/season` to check, or `/config league` to switch.")
                .await?;
        }
        RemainingResult::NoRegistrations => {
            ctx.say("No teams assigned yet. Use `/claim` to pick a nation.")
                .await?;
        }
        RemainingResult::Report(report) => {
            let embed = serenity::CreateEmbed::default()
                .title("World Cup teams remaining")
                .field(
                    "Still in",
                    remaining::format_grouped_field(
                        &report.still_in_by_user,
                        &report.unassigned_still_in,
                    ),
                    false,
                )
                .field(
                    "Eliminated",
                    remaining::format_grouped_field(
                        &report.eliminated_by_user,
                        &report.unassigned_eliminated,
                    ),
                    false,
                );

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
    }

    Ok(())
}

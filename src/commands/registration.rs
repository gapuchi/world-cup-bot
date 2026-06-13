use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    registration::{self, SeasonTeamsList, UnclaimedTeams},
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Assign a World Cup nation to a member during an active draft
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub async fn assign(
    ctx: Context<'_>,
    #[description = "Member to assign the team to"] user: serenity::Member,
    #[description = "World Cup team name, abbreviation, or code (e.g. Brazil, BRA)"] team: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message = registration::assign_for_user(
        ctx.data(),
        guild_id,
        user.user.id.get(),
        &team,
        &user.mention().to_string(),
    )
    .await?;
    ctx.say(message).await?;
    Ok(())
}

/// Remove a team assignment from a member during an active draft
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub async fn unassign(
    ctx: Context<'_>,
    #[description = "Member to unassign the team from"] user: serenity::Member,
    #[description = "World Cup team name, abbreviation, or code (e.g. Brazil, BRA)"] team: String,
) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message = registration::unassign_for_user(
        ctx.data(),
        guild_id,
        user.user.id.get(),
        &team,
        &user.mention().to_string(),
    )
    .await?;
    ctx.say(message).await?;
    Ok(())
}

/// Show the teams you have claimed
#[poise::command(prefix_command, slash_command, guild_only, rename = "team")]
pub async fn my_team(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message =
        registration::my_team_message(ctx.data(), guild_id, ctx.author().id.get()).await?;

    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// List all team assignments in this server
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn teams(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    match registration::list_season_teams(ctx.data(), guild_id).await? {
        SeasonTeamsList::Empty => {
            ctx.say("No teams assigned yet. An admin can start a draft with `/draft start`.")
                .await?;
        }
        SeasonTeamsList::ByUser(assignments) => {
            let lines: Vec<String> = assignments
                .iter()
                .map(|(user_id, teams)| {
                    format!("<@{}> — **{}**", user_id, teams.join("**, **"))
                })
                .collect();

            let embed = serenity::CreateEmbed::default()
                .title("World Cup team assignments")
                .description(lines.join("\n"));

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
    }

    Ok(())
}

/// List World Cup teams that have not been assigned yet
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn unclaimed(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = guild_id(&ctx)?;
    match registration::unclaimed_teams(ctx.data(), guild_id).await? {
        UnclaimedTeams::AllClaimed => {
            ctx.say("Every World Cup team has been assigned.").await?;
        }
        UnclaimedTeams::Available(names) => {
            let embed = serenity::CreateEmbed::default()
                .title("Unassigned World Cup teams")
                .description(
                    names
                        .iter()
                        .map(|name| format!("**{name}**"))
                        .collect::<Vec<_>>()
                        .join(", "),
                );

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
    }

    Ok(())
}

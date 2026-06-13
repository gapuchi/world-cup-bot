use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    registration::{self, PoolTeamsList, UnclaimedTeams},
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Claim a World Cup nation for yourself
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn claim(
    ctx: Context<'_>,
    #[description = "World Cup team name, abbreviation, or code (e.g. Brazil, BRA)"] team: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let message =
        registration::claim_for_user(ctx.data(), guild_id, ctx.author().id.get(), &team).await?;
    ctx.say(message).await?;
    Ok(())
}

/// Claim a World Cup nation for another member
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn assign(
    ctx: Context<'_>,
    #[description = "Member to claim the team for"] user: serenity::Member,
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

/// Remove a claimed team
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn unclaim(
    ctx: Context<'_>,
    #[description = "World Cup team name, abbreviation, or code (e.g. Brazil, BRA)"] team: String,
) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let message = registration::unclaim_for_user(
        ctx.data(),
        guild_id,
        ctx.author().id.get(),
        &team,
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
    match registration::list_pool_teams(ctx.data(), guild_id).await? {
        PoolTeamsList::Empty => {
            ctx.say("No teams claimed yet. Use `/claim` to pick a nation.")
                .await?;
        }
        PoolTeamsList::ByUser(assignments) => {
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

/// List World Cup teams that have not been claimed yet
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn unclaimed(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = guild_id(&ctx)?;
    match registration::unclaimed_teams(ctx.data(), guild_id).await? {
        UnclaimedTeams::AllClaimed => {
            ctx.say("Every World Cup team has been claimed.").await?;
        }
        UnclaimedTeams::Available(names) => {
            let embed = serenity::CreateEmbed::default()
                .title("Unclaimed World Cup teams")
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

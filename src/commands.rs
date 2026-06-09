use std::collections::{HashMap, HashSet};

use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    api::{self, find_players, find_team},
    db,
    scoring::{DRAW_POINTS, LOSS_POINTS, WIN_POINTS},
    types::{Context, Error},
};

/// Health check
#[poise::command(prefix_command, slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("pong").await?;
    Ok(())
}

/// Re-register slash commands in this server
#[poise::command(prefix_command, slash_command)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

/// List commands and how to use them
#[poise::command(prefix_command, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    let config = poise::builtins::HelpConfiguration {
        extra_text_at_bottom: "Use `/help <command>` for details on a specific command.",
        include_description: true,
        show_subcommands: true,
        ..Default::default()
    };
    poise::builtins::help(ctx, command.as_deref(), config).await?;
    Ok(())
}

/// Set the channel for match result announcements
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "channel",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_channel(
    ctx: Context<'_>,
    #[description = "Channel for match result announcements"] channel: serenity::GuildChannel,
) -> Result<(), Error> {
    let channel_id = channel.id.get();

    {
        let conn = ctx.data().db.lock().await;
        db::set_announce_channel(&conn, channel_id)?;
    }

    ctx.say(format!(
        "Match announcements will be posted in {}.",
        channel.mention()
    ))
    .await?;
    Ok(())
}

/// Claim a World Cup nation for yourself
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn claim(
    ctx: Context<'_>,
    #[description = "World Cup team name, abbreviation, or code (e.g. Brazil, BRA)"] team: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let user_id = ctx.author().id.get();

    let teams = api::fetch_teams(&ctx.data().http, &ctx.data().api_token).await?;
    let Some(selected) = find_team(&teams, &team) else {
        ctx.say(format!(
            "Couldn't find a World Cup team matching \"{team}\". Try the full name or three-letter code (e.g. BRA)."
        ))
        .await?;
        return Ok(());
    };

    {
        let conn = ctx.data().db.lock().await;
        if let Some(existing) = db::get_registration_by_team(&conn, selected.id)? {
            if existing.user_id != user_id {
                ctx.say(format!(
                    "{} is already claimed by <@{}>.",
                    selected.name, existing.user_id
                ))
                .await?;
                return Ok(());
            }
        }

        db::register_team(&conn, user_id, selected.id, &selected.name)?;
    }

    ctx.say(format!(
        "You've claimed **{}**. You'll earn points when they play.",
        selected.name
    ))
    .await?;
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

    let user_id = user.user.id.get();

    let teams = api::fetch_teams(&ctx.data().http, &ctx.data().api_token).await?;
    let Some(selected) = find_team(&teams, &team) else {
        ctx.say(format!(
            "Couldn't find a World Cup team matching \"{team}\". Try the full name or three-letter code (e.g. BRA)."
        ))
        .await?;
        return Ok(());
    };

    {
        let conn = ctx.data().db.lock().await;
        if let Some(existing) = db::get_registration_by_team(&conn, selected.id)? {
            if existing.user_id != user_id {
                ctx.say(format!(
                    "**{}** is already claimed by <@{}>.",
                    selected.name, existing.user_id
                ))
                .await?;
                return Ok(());
            }
        }

        db::register_team(&conn, user_id, selected.id, &selected.name)?;
    }

    ctx.say(format!(
        "**{}** has been claimed by {}.",
        selected.name,
        user.mention()
    ))
    .await?;
    Ok(())
}

/// Remove a claimed team
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn unclaim(
    ctx: Context<'_>,
    #[description = "World Cup team name, abbreviation, or code (e.g. Brazil, BRA)"] team: String,
) -> Result<(), Error> {
    let user_id = ctx.author().id.get();

    let teams = api::fetch_teams(&ctx.data().http, &ctx.data().api_token).await?;
    let Some(selected) = find_team(&teams, &team) else {
        ctx.say(format!(
            "Couldn't find a World Cup team matching \"{team}\". Try the full name or three-letter code (e.g. BRA)."
        ))
        .await?;
        return Ok(());
    };

    let removed = {
        let conn = ctx.data().db.lock().await;
        db::unregister_team(&conn, user_id, selected.id)?
    };

    if removed {
        ctx.say("That team has been unclaimed.").await?;
    } else {
        ctx.say("You haven't claimed that team. Use `/team` to see your teams.")
            .await?;
    }

    Ok(())
}

/// Designate a tie-breaker player from your claimed teams' squads
#[poise::command(prefix_command, slash_command, guild_only, rename = "pick-player")]
pub async fn pick_player(
    ctx: Context<'_>,
    #[description = "Player name from one of your claimed teams (e.g. Mbappé)"] player: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let user_id = ctx.author().id.get();

    let registrations = {
        let conn = ctx.data().db.lock().await;
        db::list_user_registrations(&conn, user_id)?
    };

    if registrations.is_empty() {
        ctx.say("Claim a team first with `/claim`, then pick a player from that squad.")
            .await?;
        return Ok(());
    }

    let teams: Vec<(i64, String)> = registrations
        .iter()
        .map(|registration| (registration.team_id, registration.team_name.clone()))
        .collect();

    let squad = api::fetch_squads_for_teams(&ctx.data().http, &ctx.data().api_token, &teams).await?;
    let matches = find_players(&squad, &player);

    let message = match matches.as_slice() {
        [] => format!(
            "Couldn't find a player matching \"{player}\" on your claimed teams. Try a more specific name."
        ),
        [selected] => {
            let conn = ctx.data().db.lock().await;
            db::set_tiebreaker_pick(
                &conn,
                user_id,
                selected.player_id,
                &selected.player_name,
                selected.team_id,
                &selected.team_name,
            )?;
            format!(
                "Tie-breaker player set to **{}** ({})",
                selected.player_name, selected.team_name
            )
        }
        _ => {
            let options: Vec<String> = matches
                .iter()
                .take(10)
                .map(|candidate| format!("**{}** ({})", candidate.player_name, candidate.team_name))
                .collect();
            format!(
                "Several players match \"{player}\". Be more specific:\n{}",
                options.join("\n")
            )
        }
    };

    ctx.say(message).await?;
    Ok(())
}

/// Show the teams you have claimed
#[poise::command(prefix_command, slash_command, guild_only, rename = "team")]
pub async fn my_team(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.get();

    let (registrations, pick, tiebreaker_goals) = {
        let conn = ctx.data().db.lock().await;
        let registrations = db::list_user_registrations(&conn, user_id)?;
        let pick = db::get_tiebreaker_pick(&conn, user_id)?;
        let tiebreaker_goals = db::tiebreaker_goals_for_user(&conn, user_id)?;
        (registrations, pick, tiebreaker_goals)
    };

    let mut message = match registrations.as_slice() {
        [] => "You haven't claimed any teams yet. Use `/claim` to pick one.".into(),
        [registration] => format!("You're representing **{}**.", registration.team_name),
        _ => {
            let teams: Vec<&str> = registrations
                .iter()
                .map(|registration| registration.team_name.as_str())
                .collect();
            format!("You're representing: **{}**.", teams.join("**, **"))
        }
    };

    if let Some(pick) = pick {
        message.push_str(&format!(
            "\n\nTie-breaker: **{}** ({}) — **{}** goals",
            pick.player_name, pick.team_name, tiebreaker_goals
        ));
    } else if !registrations.is_empty() {
        message.push_str("\n\nTie-breaker: none — use `/pick-player` to designate one.");
    }

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
    let registrations = {
        let conn = ctx.data().db.lock().await;
        db::list_registrations(&conn)?
    };

    if registrations.is_empty() {
        ctx.say("No teams claimed yet. Use `/claim` to pick a nation.")
            .await?;
        return Ok(());
    }

    let mut by_user: HashMap<u64, Vec<String>> = HashMap::new();
    for registration in &registrations {
        by_user
            .entry(registration.user_id)
            .or_default()
            .push(registration.team_name.clone());
    }

    let mut user_ids: Vec<u64> = by_user.keys().copied().collect();
    user_ids.sort();

    let lines: Vec<String> = user_ids
        .iter()
        .map(|user_id| {
            let teams = &by_user[user_id];
            format!("<@{}> — **{}**", user_id, teams.join("**, **"))
        })
        .collect();

    let embed = serenity::CreateEmbed::default()
        .title("World Cup team assignments")
        .description(lines.join("\n"));

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// List World Cup teams that have not been claimed yet
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn unclaimed(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let teams = api::fetch_teams(&ctx.data().http, &ctx.data().api_token).await?;

    let claimed_team_ids = {
        let conn = ctx.data().db.lock().await;
        db::list_registrations(&conn)?
            .iter()
            .map(|registration| registration.team_id)
            .collect::<HashSet<_>>()
    };

    let mut unclaimed_names: Vec<String> = teams
        .iter()
        .filter(|team| !claimed_team_ids.contains(&team.id))
        .map(|team| team.name.clone())
        .collect();
    unclaimed_names.sort();

    if unclaimed_names.is_empty() {
        ctx.say("Every World Cup team has been claimed.").await?;
        return Ok(());
    }

    let embed = serenity::CreateEmbed::default()
        .title("Unclaimed World Cup teams")
        .description(
            unclaimed_names
                .iter()
                .map(|name| format!("**{name}**"))
                .collect::<Vec<_>>()
                .join(", "),
        );

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Show the points leaderboard
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn standings(ctx: Context<'_>) -> Result<(), Error> {
    let rows = {
        let conn = ctx.data().db.lock().await;
        db::get_standings(&conn)?
    };

    if rows.is_empty() {
        ctx.say("No standings yet — claim teams with `/claim` first.")
            .await?;
        return Ok(());
    }

    let lines: Vec<String> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let mut line = format!(
                "{}. <@{}> — **{}** pts",
                index + 1,
                row.user_id,
                row.points,
            );
            for (team_name, points) in &row.teams {
                line.push_str(&format!("\n   • **{team_name}** — {points} pts"));
            }
            match &row.tiebreaker_player {
                Some(player) => line.push_str(&format!(
                    "\n   • Tie-breaker: **{player}** — {} goals",
                    row.tiebreaker_goals
                )),
                None => line.push_str(&format!(
                    "\n   • Tie-breaker — {} goals",
                    row.tiebreaker_goals
                )),
            }
            line
        })
        .collect();

    let embed = serenity::CreateEmbed::default()
        .title("World Cup standings")
        .description(lines.join("\n"))
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Win {WIN_POINTS} · Draw {DRAW_POINTS} · Loss {LOSS_POINTS} · TB = tie-breaker goals"
        )));

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Server configuration
#[poise::command(prefix_command, slash_command, subcommands("config_channel"))]
pub async fn config(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

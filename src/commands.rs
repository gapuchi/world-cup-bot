use std::collections::{HashMap, HashSet};

use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    db::{
        GuildConfig, Pool, Registration, Season, SeasonDisplay, WcTiebreakerPick,
        league_exists, league_supports_pool,
    },
    soccar::{fetch_squads_for_teams, find_players, find_team},
    scoring::{DRAW_POINTS, LOSS_POINTS, WIN_POINTS},
    standings::{self, StandingRow},
    types::{Context, Error},
};

fn guild_id(ctx: &Context<'_>) -> Result<u64, Error> {
    Ok(ctx
        .guild_id()
        .ok_or("This command must be used in a server.")?
        .get())
}

fn active_competition(conn: &rusqlite::Connection, pool: &Pool) -> rusqlite::Result<String> {
    let season = Season::get(conn, pool.season_id)?.ok_or_else(|| {
        rusqlite::Error::QueryReturnedNoRows
    })?;
    Ok(season.external_season_id.unwrap_or_else(|| "WC".into()))
}

/// Health check
#[poise::command(prefix_command, slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("pong").await?;
    Ok(())
}

/// Show the bot version
#[poise::command(prefix_command, slash_command)]
pub async fn version(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(format!("world-cup-bot **v{}**", env!("CARGO_PKG_VERSION")))
        .await?;
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

/// Set the active league for commands
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "league",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_league(
    ctx: Context<'_>,
    #[description = "League slug (e.g. wc)"] league: String,
) -> Result<(), Error> {
    let slug = league.trim().to_lowercase();

    {
        let conn = ctx.data().db.lock().await;
        if !league_exists(&conn, &slug)? {
            ctx.say(format!("Unknown league \"{slug}\".")).await?;
            return Ok(());
        }
    }

    if !league_supports_pool(&slug) {
        ctx.say(format!("League \"{slug}\" is not supported yet.")).await?;
        return Ok(());
    }

    let message = {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let pool = Pool::get_or_create_for_league(&conn, guild_id, &slug)?;
        GuildConfig::set_default_pool_id(&conn, guild_id, pool.id)?;
        let display = SeasonDisplay::for_pool(&conn, pool.id)?;
        format!(
            "Active league set to **{}** — tracking **{}** (`{}`).",
            display.league_name, display.name, display.slug
        )
    };

    ctx.say(message).await?;
    Ok(())
}

/// List configured league pools
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "leagues",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_leagues(ctx: Context<'_>) -> Result<(), Error> {
    let (default_pool_id, pools) = {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let default_pool_id = GuildConfig::get(&conn, guild_id)?.map(|config| config.default_pool_id);
        let pools = Pool::list_with_league(&conn, guild_id)?;
        (default_pool_id, pools)
    };

    if pools.is_empty() {
        ctx.say("No league pools configured. Use `/config league wc` to enable one.")
            .await?;
        return Ok(());
    }

    let lines: Vec<String> = pools
        .iter()
        .map(|entry| {
            let active = default_pool_id == Some(entry.pool.id);
            let marker = if active { " (active)" } else { "" };
            format!(
                "**{}** (`{}`){marker}",
                entry.league_name, entry.league_slug
            )
        })
        .collect();

    ctx.say(lines.join("\n")).await?;
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
        let guild_id = guild_id(&ctx)?;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        Pool::set_announce_channel(&conn, pool.id, channel_id)?;
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

    let guild_id = guild_id(&ctx)?;
    let user_id = ctx.author().id.get();
    let competition = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        active_competition(&conn, &pool)?
    };
    let teams = ctx.data().soccar_api().fetch_teams(&competition).await?;
    let Some(selected) = find_team(&teams, &team) else {
        ctx.say(format!(
            "Couldn't find a World Cup team matching \"{team}\". Try the full name or three-letter code (e.g. BRA)."
        ))
        .await?;
        return Ok(());
    };

    {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        if let Some(existing) = Registration::get_by_team(&conn, pool.id, selected.id)?
            && existing.user_id != user_id
        {
            ctx.say(format!(
                "{} is already claimed by <@{}>.",
                selected.name, existing.user_id
            ))
            .await?;
            return Ok(());
        }

        Registration::upsert(&conn, pool.id, user_id, selected.id, &selected.name)?;
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

    let guild_id = guild_id(&ctx)?;
    let user_id = user.user.id.get();
    let competition = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        active_competition(&conn, &pool)?
    };
    let teams = ctx.data().soccar_api().fetch_teams(&competition).await?;
    let Some(selected) = find_team(&teams, &team) else {
        ctx.say(format!(
            "Couldn't find a World Cup team matching \"{team}\". Try the full name or three-letter code (e.g. BRA)."
        ))
        .await?;
        return Ok(());
    };

    {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        if let Some(existing) = Registration::get_by_team(&conn, pool.id, selected.id)?
            && existing.user_id != user_id
        {
            ctx.say(format!(
                "**{}** is already claimed by <@{}>.",
                selected.name, existing.user_id
            ))
            .await?;
            return Ok(());
        }

        Registration::upsert(&conn, pool.id, user_id, selected.id, &selected.name)?;
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
    let guild_id = guild_id(&ctx)?;
    let user_id = ctx.author().id.get();
    let competition = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        active_competition(&conn, &pool)?
    };
    let teams = ctx.data().soccar_api().fetch_teams(&competition).await?;
    let Some(selected) = find_team(&teams, &team) else {
        ctx.say(format!(
            "Couldn't find a World Cup team matching \"{team}\". Try the full name or three-letter code (e.g. BRA)."
        ))
        .await?;
        return Ok(());
    };

    let removed = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        Registration::delete(&conn, pool.id, user_id, selected.id)?
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

    let guild_id = guild_id(&ctx)?;
    let user_id = ctx.author().id.get();
    let registrations = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        Registration::list_for_user(&conn, pool.id, user_id)?
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

    let squad = fetch_squads_for_teams(&ctx.data().soccar_api(), &teams).await?;
    let matches = find_players(&squad, &player);

    let message = match matches.as_slice() {
        [] => format!(
            "Couldn't find a player matching \"{player}\" on your claimed teams. Try a more specific name."
        ),
        [selected] => {
            let conn = ctx.data().db.lock().await;
            let pool = Pool::default_for_guild(&conn, guild_id)?;
            WcTiebreakerPick::upsert(
                &conn,
                pool.id,
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
    let guild_id = guild_id(&ctx)?;
    let user_id = ctx.author().id.get();
    let (registrations, pick, tiebreaker_goals) = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        let registrations = Registration::list_for_user(&conn, pool.id, user_id)?;
        let pick = WcTiebreakerPick::get_for_user(&conn, pool.id, user_id)?;
        let tiebreaker_goals = standings::tiebreaker_goals_for_user(&conn, pool.id, user_id)?;
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
    let guild_id = guild_id(&ctx)?;
    let registrations = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        Registration::list_for_pool(&conn, pool.id)?
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

    let guild_id = guild_id(&ctx)?;
    let competition = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        active_competition(&conn, &pool)?
    };
    let teams = ctx.data().soccar_api().fetch_teams(&competition).await?;

    let claimed_team_ids = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        Registration::list_for_pool(&conn, pool.id)?
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
fn format_standing_summary(rank: usize, row: &StandingRow) -> String {
    format!(
        "**{rank}** · <@{}> — **{}** pts",
        row.user_id,
        row.points,
    )
}

fn format_standing_detail(rank: usize, row: &StandingRow) -> String {
    let mut line = format_standing_summary(rank, row);
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
}

#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn standings(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let rows = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        standings::get_standings(&conn, pool.id)?
    };

    if rows.is_empty() {
        ctx.say("No standings yet — claim teams with `/claim` first.")
            .await?;
        return Ok(());
    }

    let footer = format!(
        "Win {WIN_POINTS} · Draw {DRAW_POINTS} · Loss {LOSS_POINTS} · TB = tie-breaker goals"
    );

    let ranks = standings::standings_ranks(&rows);

    let summary_lines: Vec<String> = rows
        .iter()
        .zip(&ranks)
        .map(|(row, rank)| format_standing_summary(*rank, row))
        .collect();

    let detail_lines: Vec<String> = rows
        .iter()
        .zip(&ranks)
        .map(|(row, rank)| format_standing_detail(*rank, row))
        .collect();

    let summary_embed = serenity::CreateEmbed::default()
        .title("World Cup standings")
        .description(summary_lines.join("\n"))
        .footer(serenity::CreateEmbedFooter::new(&footer));

    let reply = ctx
        .send(poise::CreateReply::default().embed(summary_embed))
        .await?;
    let message = reply.into_message().await?;

    let detail_embed = serenity::CreateEmbed::default()
        .title("Standings breakdown")
        .description(detail_lines.join("\n\n"))
        .footer(serenity::CreateEmbedFooter::new(footer));

    let thread = message
        .channel_id
        .create_thread_from_message(
            ctx.serenity_context(),
            message.id,
            serenity::CreateThread::new("Standings breakdown"),
        )
        .await?;

    thread
        .id
        .send_message(
            ctx.serenity_context(),
            serenity::CreateMessage::new().embed(detail_embed),
        )
        .await?;

    Ok(())
}

/// Show the active season
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn season(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let season = {
        let conn = ctx.data().db.lock().await;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        SeasonDisplay::for_pool(&conn, pool.id)?
    };

    ctx.say(format!(
        "This bot is tracking **{}** (`{}`) for **{}**.",
        season.name, season.slug, season.league_name
    ))
    .await?;
    Ok(())
}

/// Server configuration
#[poise::command(
    prefix_command,
    slash_command,
    subcommands("config_channel", "config_league", "config_leagues")
)]
pub async fn config(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

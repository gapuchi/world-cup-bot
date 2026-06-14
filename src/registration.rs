use std::collections::{HashMap, HashSet};

use crate::{
    db::{Draft, DraftStatus, Registration, Season, NFL_LEAGUE_SLUG},
    draft::{self, TurnChange},
    gridiron::find_team as find_nfl_team,
    soccar::find_team as find_wc_team,
    standings,
    types::{Data, Error},
};

struct SelectedTeam {
    name: String,
}

async fn fetch_teams(
    data: &Data,
    guild_id: u64,
    league_slug: &str,
) -> Result<Vec<(i64, String, Option<String>)>, Error> {
    if league_slug == NFL_LEAGUE_SLUG {
        let teams = data.espn_api().fetch_teams().await?;
        Ok(teams
            .into_iter()
            .map(|team| (team.id, team.name, team.abbreviation))
            .collect())
    } else {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let league_slug = Season::league_slug_for(&conn, season.id)?;
        let competition = crate::db::league_competition_code(&league_slug);
        drop(conn);
        let teams = data.soccar_api().fetch_teams(&competition).await?;
        Ok(teams
            .into_iter()
            .map(|team| (team.id, team.name, team.tla))
            .collect())
    }
}

fn find_team(
    teams: &[(i64, String, Option<String>)],
    query: &str,
    league_slug: &str,
) -> Option<(i64, String)> {
    if league_slug == NFL_LEAGUE_SLUG {
        let nfl_teams: Vec<crate::api::NflTeam> = teams
            .iter()
            .map(|(id, name, abbr)| crate::api::NflTeam {
                id: *id,
                name: name.clone(),
                abbreviation: abbr.clone(),
            })
            .collect();
        find_nfl_team(&nfl_teams, query).map(|team| (team.id, team.name.clone()))
    } else {
        let wc_teams: Vec<crate::api::Team> = teams
            .iter()
            .map(|(id, name, abbr)| crate::api::Team {
                id: *id,
                name: name.clone(),
                short_name: None,
                tla: abbr.clone(),
            })
            .collect();
        find_wc_team(&wc_teams, query).map(|team| (team.id, team.name.clone()))
    }
}

fn team_not_found_message(team_query: &str, league_slug: &str) -> String {
    if league_slug == NFL_LEAGUE_SLUG {
        format!(
            "Couldn't find an NFL team matching \"{team_query}\". Try the full name or abbreviation (e.g. PHI)."
        )
    } else {
        format!(
            "Couldn't find a World Cup team matching \"{team_query}\". Try the full name or three-letter code (e.g. BRA)."
        )
    }
}

fn no_active_draft_message() -> String {
    "No active draft. An admin can start one with `/draft start`.".into()
}

fn draft_finished_message() -> String {
    "The draft is finished. Rosters are locked.".into()
}

fn not_your_turn_message(current_picker: u64) -> String {
    format!("It's <@{current_picker}>'s turn. Wait for `/draft pick`.")
}

async fn require_active_draft(data: &Data, guild_id: u64) -> Result<(), String> {
    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id).map_err(|error| error.to_string())?;
    match Draft::get(&conn, season.id).map_err(|error| error.to_string())? {
        None => Err(no_active_draft_message()),
        Some(draft) if draft.status == DraftStatus::Complete => Err(draft_finished_message()),
        Some(draft) if draft.status == DraftStatus::Active => {
            if draft.current_pick >= draft.total_picks {
                Err(draft_finished_message())
            } else {
                Ok(())
            }
        }
        Some(_) => Err(no_active_draft_message()),
    }
}

async fn register_team(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    team_query: &str,
) -> Result<Result<SelectedTeam, String>, Error> {
    let league_slug = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Season::league_slug_for(&conn, season.id)?
    };

    let api_teams = fetch_teams(data, guild_id, &league_slug).await?;
    let Some((id, name)) = find_team(&api_teams, team_query, &league_slug) else {
        return Ok(Err(team_not_found_message(team_query, &league_slug)));
    };

    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    if let Some(existing) = Registration::get_by_team(&conn, season.id, id)?
        && existing.user_id != user_id
    {
        return Ok(Err(format!(
            "**{}** is already taken by <@{}>.",
            name, existing.user_id
        )));
    }

    Registration::upsert(&conn, season.id, user_id, id, &name)?;
    Ok(Ok(SelectedTeam { name }))
}

pub async fn pick_for_user(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    team_query: &str,
) -> Result<(String, Option<TurnChange>), Error> {
    {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let Some(draft) = Draft::get(&conn, season.id)? else {
            return Ok((no_active_draft_message(), None));
        };
        match draft.status {
            DraftStatus::Complete => return Ok((draft_finished_message(), None)),
            DraftStatus::Active => {}
        }
        let Some(current_picker) = Draft::current_picker(&conn, season.id)? else {
            return Ok((draft_finished_message(), None));
        };
        if user_id != current_picker {
            return Ok((not_your_turn_message(current_picker), None));
        }
    }

    let selected = match register_team(data, guild_id, user_id, team_query).await? {
        Ok(team) => team,
        Err(message) => return Ok((message, None)),
    };

    let turn_change = draft::advance_after_pick(data, guild_id).await?;
    let mut message = format!(
        "You picked **{}**. You'll earn points when they play.",
        selected.name
    );
    if turn_change.completed {
        message.push_str("\n\nDraft complete — all picks are in. Rosters are locked.");
    } else if let Some(next_picker) = turn_change.next_picker {
        message.push_str(&format!("\n\nNext up: <@{next_picker}>."));
    }

    Ok((message, Some(turn_change)))
}

pub async fn assign_for_user(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    team_query: &str,
    assignee_mention: &str,
) -> Result<String, Error> {
    if let Err(message) = require_active_draft(data, guild_id).await {
        return Ok(message);
    }

    let selected = match register_team(data, guild_id, user_id, team_query).await? {
        Ok(team) => team,
        Err(message) => return Ok(message),
    };

    Ok(format!(
        "**{}** assigned to {}.",
        selected.name, assignee_mention
    ))
}

pub async fn unassign_for_user(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    team_query: &str,
    assignee_mention: &str,
) -> Result<String, Error> {
    if let Err(message) = require_active_draft(data, guild_id).await {
        return Ok(message);
    }

    let league_slug = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Season::league_slug_for(&conn, season.id)?
    };

    let api_teams = fetch_teams(data, guild_id, &league_slug).await?;
    let Some((id, name)) = find_team(&api_teams, team_query, &league_slug) else {
        return Ok(team_not_found_message(team_query, &league_slug));
    };

    let removed = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Registration::delete(&conn, season.id, user_id, id)?
    };

    Ok(if removed {
        format!("**{}** unassigned from {}.", name, assignee_mention)
    } else {
        format!("{assignee_mention} doesn't have **{}**.", name)
    })
}

pub async fn my_team_message(
    data: &Data,
    guild_id: u64,
    user_id: u64,
) -> Result<String, Error> {
    let (registrations, tiebreaker_player, tiebreaker_team, tiebreaker_stat, league_slug) = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let league_slug = Season::league_slug_for(&conn, season.id)?;
        let registrations = Registration::list_for_user(&conn, season.id, user_id)?;
        let tiebreaker_stat = standings::tiebreaker_stat_for_user(&conn, season.id, user_id)?;
        let (tiebreaker_player, tiebreaker_team) = if league_slug == NFL_LEAGUE_SLUG {
            let pick = crate::db::NflTiebreakerPick::get_for_user(&conn, season.id, user_id)?;
            (
                pick.as_ref().map(|p| p.player_name.clone()),
                pick.map(|p| p.team_name),
            )
        } else {
            let pick = crate::db::WcTiebreakerPick::get_for_user(&conn, season.id, user_id)?;
            (
                pick.as_ref().map(|p| p.player_name.clone()),
                pick.map(|p| p.team_name),
            )
        };
        (
            registrations,
            tiebreaker_player,
            tiebreaker_team,
            tiebreaker_stat,
            league_slug,
        )
    };

    let stat_label = if league_slug == NFL_LEAGUE_SLUG {
        "touchdowns"
    } else {
        "goals"
    };

    let mut message = match registrations.as_slice() {
        [] => "You don't have any teams yet. They'll show up here during a draft.".into(),
        [registration] => format!("You're representing **{}**.", registration.team_name),
        _ => {
            let teams: Vec<&str> = registrations
                .iter()
                .map(|registration| registration.team_name.as_str())
                .collect();
            format!("You're representing: **{}**.", teams.join("**, **"))
        }
    };

    if let (Some(player), Some(team)) = (tiebreaker_player, tiebreaker_team) {
        message.push_str(&format!(
            "\n\nTie-breaker: **{}** ({}) — **{}** {stat_label}",
            player, team, tiebreaker_stat
        ));
    } else if !registrations.is_empty() {
        message.push_str("\n\nTie-breaker: none — use `/pick-player` to designate one.");
    }

    Ok(message)
}

pub enum SeasonTeamsList {
    Empty,
    ByUser(Vec<(u64, Vec<String>)>),
}

pub async fn list_season_teams(data: &Data, guild_id: u64) -> Result<SeasonTeamsList, Error> {
    let registrations = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Registration::list_for_season(&conn, season.id)?
    };

    if registrations.is_empty() {
        return Ok(SeasonTeamsList::Empty);
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

    let assignments = user_ids
        .into_iter()
        .map(|user_id| (user_id, by_user.remove(&user_id).unwrap_or_default()))
        .collect();

    Ok(SeasonTeamsList::ByUser(assignments))
}

pub enum UnclaimedTeams {
    AllClaimed,
    Available(Vec<String>),
}

pub async fn unclaimed_teams(data: &Data, guild_id: u64) -> Result<UnclaimedTeams, Error> {
    let league_slug = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Season::league_slug_for(&conn, season.id)?
    };

    let api_teams = fetch_teams(data, guild_id, &league_slug).await?;

    let claimed_team_ids = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Registration::list_for_season(&conn, season.id)?
            .iter()
            .map(|registration| registration.team_id)
            .collect::<HashSet<_>>()
    };

    let mut unclaimed_names: Vec<String> = api_teams
        .iter()
        .filter(|(id, _, _)| !claimed_team_ids.contains(id))
        .map(|(_, name, _)| name.clone())
        .collect();
    unclaimed_names.sort();

    Ok(if unclaimed_names.is_empty() {
        UnclaimedTeams::AllClaimed
    } else {
        UnclaimedTeams::Available(unclaimed_names)
    })
}

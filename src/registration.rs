use std::collections::{HashMap, HashSet};

use crate::{
    db::{Draft, DraftStatus, Registration, Season, league_competition_code},
    draft::{self, TurnChange},
    soccar::find_team,
    standings,
    types::{Data, Error},
};

fn active_competition(conn: &rusqlite::Connection, season: &Season) -> rusqlite::Result<String> {
    let league_slug = Season::league_slug_for(conn, season.id)?;
    Ok(league_competition_code(&league_slug))
}

async fn fetch_competition_teams(
    data: &Data,
    guild_id: u64,
) -> Result<Vec<crate::api::Team>, Error> {
    let competition = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        active_competition(&conn, &season)?
    };
    Ok(data.soccar_api().fetch_teams(&competition).await?)
}

fn team_not_found_message(team_query: &str) -> String {
    format!(
        "Couldn't find a World Cup team matching \"{team_query}\". Try the full name or three-letter code (e.g. BRA)."
    )
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
) -> Result<Result<crate::api::Team, String>, Error> {
    let api_teams = fetch_competition_teams(data, guild_id).await?;
    let Some(selected) = find_team(&api_teams, team_query) else {
        return Ok(Err(team_not_found_message(team_query)));
    };

    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    if let Some(existing) = Registration::get_by_team(&conn, season.id, selected.id)?
        && existing.user_id != user_id
    {
        return Ok(Err(format!(
            "**{}** is already taken by <@{}>.",
            selected.name, existing.user_id
        )));
    }

    Registration::upsert(&conn, season.id, user_id, selected.id, &selected.name)?;
    Ok(Ok(selected.clone()))
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

    let api_teams = fetch_competition_teams(data, guild_id).await?;
    let Some(selected) = find_team(&api_teams, team_query) else {
        return Ok(team_not_found_message(team_query));
    };

    let removed = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Registration::delete(&conn, season.id, user_id, selected.id)?
    };

    Ok(if removed {
        format!("**{}** unassigned from {}.", selected.name, assignee_mention)
    } else {
        format!("{assignee_mention} doesn't have **{}**.", selected.name)
    })
}

pub async fn my_team_message(
    data: &Data,
    guild_id: u64,
    user_id: u64,
) -> Result<String, Error> {
    let (registrations, pick, tiebreaker_goals) = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let registrations = Registration::list_for_user(&conn, season.id, user_id)?;
        let pick = crate::db::WcTiebreakerPick::get_for_user(&conn, season.id, user_id)?;
        let tiebreaker_goals = standings::tiebreaker_goals_for_user(&conn, season.id, user_id)?;
        (registrations, pick, tiebreaker_goals)
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

    if let Some(pick) = pick {
        message.push_str(&format!(
            "\n\nTie-breaker: **{}** ({}) — **{}** goals",
            pick.player_name, pick.team_name, tiebreaker_goals
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
    let api_teams = fetch_competition_teams(data, guild_id).await?;

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
        .filter(|team| !claimed_team_ids.contains(&team.id))
        .map(|team| team.name.clone())
        .collect();
    unclaimed_names.sort();

    Ok(if unclaimed_names.is_empty() {
        UnclaimedTeams::AllClaimed
    } else {
        UnclaimedTeams::Available(unclaimed_names)
    })
}

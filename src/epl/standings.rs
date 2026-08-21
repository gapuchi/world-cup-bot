use std::collections::HashMap;

use rusqlite::Connection;

use crate::{
    db::{EplMatchResult, EplPlayerGoalTotal, EplTiebreakerPick, Registration},
    scoring,
    standings::StandingRow,
};

pub fn user_points(conn: &Connection, season_id: i64, user_id: u64) -> rusqlite::Result<i64> {
    let matches: Vec<_> = EplMatchResult::list_for_season(conn, season_id)?
        .iter()
        .map(EplMatchResult::as_finished_match)
        .collect();
    let registrations = Registration::list_for_user(conn, season_id, user_id)?;
    let team_ids: Vec<i64> = registrations.iter().map(|r| r.team_id).collect();
    Ok(scoring::points_for_teams(&team_ids, &matches))
}

pub fn tiebreaker_goals_for_user(
    conn: &Connection,
    season_id: i64,
    user_id: u64,
) -> rusqlite::Result<i64> {
    let Some(pick) = EplTiebreakerPick::get_for_user(conn, season_id, user_id)? else {
        return Ok(0);
    };
    EplPlayerGoalTotal::goals_for_player(conn, season_id, pick.player_id)
}

pub fn tiebreaker_pick_for_user(
    conn: &Connection,
    season_id: i64,
    user_id: u64,
) -> rusqlite::Result<Option<(String, String)>> {
    Ok(EplTiebreakerPick::get_for_user(conn, season_id, user_id)?.map(|pick| {
        (pick.player_name, pick.team_name)
    }))
}

pub fn clear_picks_for_team(
    conn: &Connection,
    season_id: i64,
    user_id: u64,
    team_id: i64,
) -> rusqlite::Result<()> {
    EplTiebreakerPick::delete_for_team(conn, season_id, user_id, team_id)
}

pub fn get_standings(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<StandingRow>> {
    let matches: Vec<_> = EplMatchResult::list_for_season(conn, season_id)?
        .iter()
        .map(EplMatchResult::as_finished_match)
        .collect();
    let registrations = Registration::list_for_season(conn, season_id)?;

    let mut by_user: HashMap<u64, (Vec<i64>, Vec<String>)> = HashMap::new();
    for registration in registrations {
        let entry = by_user.entry(registration.user_id).or_default();
        entry.0.push(registration.team_id);
        entry.1.push(registration.team_name);
    }

    let mut rows = Vec::with_capacity(by_user.len());
    for (user_id, (team_ids, team_names)) in by_user {
        let pick = EplTiebreakerPick::get_for_user(conn, season_id, user_id)?;
        let tiebreaker_goals = match &pick {
            Some(pick) => {
                EplPlayerGoalTotal::goals_for_player(conn, season_id, pick.player_id)?
            }
            None => 0,
        };
        let mut teams: Vec<(String, i64)> = team_ids
            .iter()
            .zip(&team_names)
            .map(|(team_id, team_name)| {
                (
                    team_name.clone(),
                    scoring::points_for_team(*team_id, &matches),
                )
            })
            .collect();
        teams.sort_by(|a, b| a.0.cmp(&b.0));
        rows.push(StandingRow {
            user_id,
            points: scoring::points_for_teams(&team_ids, &matches),
            teams,
            tiebreaker_goals,
            tiebreaker_player: pick.map(|pick| pick.player_name),
        });
    }

    rows.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then_with(|| b.tiebreaker_goals.cmp(&a.tiebreaker_goals))
            .then_with(|| a.user_id.cmp(&b.user_id))
    });
    Ok(rows)
}

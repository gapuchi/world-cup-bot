use std::collections::HashMap;

use rusqlite::Connection;

use crate::{
    db::{NflMatchResult, NflPlayerTouchdownTotal, NflTiebreakerPick, Registration},
    scoring::{self, ScoringRules},
};

use super::super::standings::StandingRow;

pub fn user_points(
    conn: &Connection,
    season_id: i64,
    user_id: u64,
    rules: &ScoringRules,
) -> rusqlite::Result<i64> {
    let matches: Vec<_> = NflMatchResult::list_for_season(conn, season_id)?
        .iter()
        .map(NflMatchResult::as_finished_match)
        .collect();
    let registrations = Registration::list_for_user(conn, season_id, user_id)?;
    let team_ids: Vec<i64> = registrations.iter().map(|r| r.team_id).collect();
    Ok(scoring::points_for_teams(rules, &team_ids, &matches))
}

pub fn tiebreaker_stat_for_user(
    conn: &Connection,
    season_id: i64,
    user_id: u64,
) -> rusqlite::Result<i64> {
    let Some(pick) = NflTiebreakerPick::get_for_user(conn, season_id, user_id)? else {
        return Ok(0);
    };
    NflPlayerTouchdownTotal::touchdowns_for_player(conn, season_id, pick.player_id)
}

pub fn get_standings(
    conn: &Connection,
    season_id: i64,
    rules: &ScoringRules,
) -> rusqlite::Result<Vec<StandingRow>> {
    let matches: Vec<_> = NflMatchResult::list_for_season(conn, season_id)?
        .iter()
        .map(NflMatchResult::as_finished_match)
        .collect();
    let registrations = Registration::list_for_season(conn, season_id)?;

    let mut by_user: HashMap<u64, (Vec<i64>, Vec<String>)> = HashMap::new();
    for registration in registrations {
        let entry = by_user.entry(registration.user_id).or_default();
        entry.0.push(registration.team_id);
        entry.1.push(registration.team_name);
    }

    let mut rows: Vec<StandingRow> = by_user
        .into_iter()
        .map(|(user_id, (team_ids, team_names))| {
            let pick = NflTiebreakerPick::get_for_user(conn, season_id, user_id)
                .ok()
                .flatten();
            let tiebreaker_stat = pick
                .as_ref()
                .map(|pick| {
                    NflPlayerTouchdownTotal::touchdowns_for_player(conn, season_id, pick.player_id)
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            let mut teams: Vec<(String, i64)> = team_ids
                .iter()
                .zip(&team_names)
                .map(|(team_id, team_name)| {
                    (
                        team_name.clone(),
                        scoring::points_for_team(rules, *team_id, &matches),
                    )
                })
                .collect();
            teams.sort_by(|a, b| a.0.cmp(&b.0));
            StandingRow {
                user_id,
                points: scoring::points_for_teams(rules, &team_ids, &matches),
                teams,
                tiebreaker_stat,
                tiebreaker_player: pick.map(|pick| pick.player_name),
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then_with(|| b.tiebreaker_stat.cmp(&a.tiebreaker_stat))
            .then_with(|| a.user_id.cmp(&b.user_id))
    });
    Ok(rows)
}

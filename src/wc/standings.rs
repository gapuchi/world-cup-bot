use rusqlite::Connection;

use crate::{
    db::{Registration, WcMatchResult, WcPlayerGoalTotal, WcTiebreakerPick},
    scoring::FinishedMatch,
    standings::{self, StandingRow},
};

pub fn user_points(conn: &Connection, season_id: i64, user_id: u64) -> rusqlite::Result<i64> {
    Ok(standings::points_for_user_teams(
        &finished_matches(conn, season_id)?,
        &Registration::list_for_user(conn, season_id, user_id)?,
    ))
}

pub fn tiebreaker_goals_for_user(
    conn: &Connection,
    season_id: i64,
    user_id: u64,
) -> rusqlite::Result<i64> {
    let Some(pick) = WcTiebreakerPick::get_for_user(conn, season_id, user_id)? else {
        return Ok(0);
    };
    WcPlayerGoalTotal::goals_for_player(conn, season_id, pick.player_id)
}

pub fn tiebreaker_pick_for_user(
    conn: &Connection,
    season_id: i64,
    user_id: u64,
) -> rusqlite::Result<Option<(String, String)>> {
    Ok(WcTiebreakerPick::get_for_user(conn, season_id, user_id)?
        .map(|pick| (pick.player_name, pick.team_name)))
}

pub fn clear_picks_for_team(
    conn: &Connection,
    season_id: i64,
    user_id: u64,
    team_id: i64,
) -> rusqlite::Result<()> {
    WcTiebreakerPick::delete_for_team(conn, season_id, user_id, team_id)
}

pub fn get_standings(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<StandingRow>> {
    standings::build_rows(
        &finished_matches(conn, season_id)?,
        &Registration::list_for_season(conn, season_id)?,
        |user_id| {
            let pick = WcTiebreakerPick::get_for_user(conn, season_id, user_id)?;
            let goals = match &pick {
                Some(pick) => {
                    WcPlayerGoalTotal::goals_for_player(conn, season_id, pick.player_id)?
                }
                None => 0,
            };
            Ok((goals, pick.map(|p| p.player_name)))
        },
    )
}

fn finished_matches(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<FinishedMatch>> {
    Ok(WcMatchResult::list_for_season(conn, season_id)?
        .iter()
        .map(WcMatchResult::as_finished_match)
        .collect())
}

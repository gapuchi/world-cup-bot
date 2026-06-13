use std::collections::HashMap;

use rusqlite::Connection;

use crate::{
    db::{Registration, WcMatchResult, WcPlayerGoalTotal, WcTiebreakerPick},
    scoring::{self, DRAW_POINTS, LOSS_POINTS, WIN_POINTS},
};

pub struct StandingRow {
    pub user_id: u64,
    pub points: i64,
    pub teams: Vec<(String, i64)>,
    pub tiebreaker_goals: i64,
    pub tiebreaker_player: Option<String>,
}

pub fn user_points(conn: &Connection, season_id: i64, user_id: u64) -> rusqlite::Result<i64> {
    let matches: Vec<_> = WcMatchResult::list_for_season(conn, season_id)?
        .iter()
        .map(WcMatchResult::as_finished_match)
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
    let Some(pick) = WcTiebreakerPick::get_for_user(conn, season_id, user_id)? else {
        return Ok(0);
    };
    WcPlayerGoalTotal::goals_for_player(conn, season_id, pick.player_id)
}

pub fn get_standings(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<StandingRow>> {
    let matches: Vec<_> = WcMatchResult::list_for_season(conn, season_id)?
        .iter()
        .map(WcMatchResult::as_finished_match)
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
            let pick = WcTiebreakerPick::get_for_user(conn, season_id, user_id)
                .ok()
                .flatten();
            let tiebreaker_goals = pick
                .as_ref()
                .map(|pick| {
                    WcPlayerGoalTotal::goals_for_player(conn, season_id, pick.player_id)
                        .unwrap_or(0)
                })
                .unwrap_or(0);
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
            StandingRow {
                user_id,
                points: scoring::points_for_teams(&team_ids, &matches),
                teams,
                tiebreaker_goals,
                tiebreaker_player: pick.map(|pick| pick.player_name),
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then_with(|| b.tiebreaker_goals.cmp(&a.tiebreaker_goals))
            .then_with(|| a.user_id.cmp(&b.user_id))
    });
    Ok(rows)
}

pub fn standings_footer() -> String {
    format!("Win {WIN_POINTS} · Draw {DRAW_POINTS} · Loss {LOSS_POINTS} · TB = tie-breaker goals")
}

pub fn format_standing_summary(rank: usize, row: &StandingRow) -> String {
    format!(
        "**{rank}** · <@{}> — **{}** pts",
        row.user_id, row.points,
    )
}

pub fn format_standing_detail(rank: usize, row: &StandingRow) -> String {
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

pub fn format_standings_summary_lines(rows: &[StandingRow], ranks: &[usize]) -> Vec<String> {
    rows.iter()
        .zip(ranks)
        .map(|(row, rank)| format_standing_summary(*rank, row))
        .collect()
}

pub fn format_standings_detail_lines(rows: &[StandingRow], ranks: &[usize]) -> Vec<String> {
    rows.iter()
        .zip(ranks)
        .map(|(row, rank)| format_standing_detail(*rank, row))
        .collect()
}

pub fn standings_ranks(rows: &[StandingRow]) -> Vec<usize> {
    let mut ranks = Vec::with_capacity(rows.len());
    let mut i = 0;
    while i < rows.len() {
        let mut j = i;
        while j + 1 < rows.len() && rows[j].points == rows[j + 1].points {
            j += 1;
        }
        let rank = i + 1;
        for _ in i..=j {
            ranks.push(rank);
        }
        i = j + 1;
    }
    ranks
}

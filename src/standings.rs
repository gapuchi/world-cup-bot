use rusqlite::Connection;

use crate::{
    db::Season,
    scoring::{self, format_points, format_rules_footer, rules_for_league},
};

pub struct StandingRow {
    pub user_id: u64,
    pub points: i64,
    pub teams: Vec<(String, i64)>,
    pub tiebreaker_stat: i64,
    pub tiebreaker_player: Option<String>,
}

pub fn user_points(conn: &Connection, season_id: i64, user_id: u64) -> rusqlite::Result<i64> {
    let league_slug = Season::league_slug_for(conn, season_id)?;
    let rules = rules_for_league(&league_slug);
    match league_slug.as_str() {
        "nfl" => crate::nfl::user_points(conn, season_id, user_id, &rules),
        _ => crate::wc::user_points(conn, season_id, user_id, &rules),
    }
}

pub fn tiebreaker_stat_for_user(
    conn: &Connection,
    season_id: i64,
    user_id: u64,
) -> rusqlite::Result<i64> {
    let league_slug = Season::league_slug_for(conn, season_id)?;
    match league_slug.as_str() {
        "nfl" => crate::nfl::tiebreaker_stat_for_user(conn, season_id, user_id),
        _ => crate::wc::tiebreaker_stat_for_user(conn, season_id, user_id),
    }
}

pub fn get_standings(conn: &Connection, season_id: i64) -> rusqlite::Result<Vec<StandingRow>> {
    let league_slug = Season::league_slug_for(conn, season_id)?;
    let rules = rules_for_league(&league_slug);
    match league_slug.as_str() {
        "nfl" => crate::nfl::get_standings(conn, season_id, &rules),
        _ => crate::wc::get_standings(conn, season_id, &rules),
    }
}

pub fn standings_footer(league_slug: &str) -> String {
    format_rules_footer(&rules_for_league(league_slug), league_slug)
}

pub fn format_standing_summary(rank: usize, row: &StandingRow, league_slug: &str) -> String {
    format!(
        "**{rank}** · <@{}> — **{}** pts",
        row.user_id,
        scoring::format_points(row.points, league_slug),
    )
}

pub fn format_standing_detail(rank: usize, row: &StandingRow, league_slug: &str) -> String {
    let mut line = format_standing_summary(rank, row, league_slug);
    for (team_name, points) in &row.teams {
        line.push_str(&format!(
            "\n   • **{team_name}** — {} pts",
            format_points(*points, league_slug)
        ));
    }
    let stat_label = if league_slug == "nfl" {
        "touchdowns"
    } else {
        "goals"
    };
    match &row.tiebreaker_player {
        Some(player) => line.push_str(&format!(
            "\n   • Tie-breaker: **{player}** — {} {stat_label}",
            row.tiebreaker_stat
        )),
        None => line.push_str(&format!(
            "\n   • Tie-breaker — {} {stat_label}",
            row.tiebreaker_stat
        )),
    }
    line
}

pub fn format_standings_summary_lines(
    rows: &[StandingRow],
    ranks: &[usize],
    league_slug: &str,
) -> Vec<String> {
    rows.iter()
        .zip(ranks)
        .map(|(row, rank)| format_standing_summary(*rank, row, league_slug))
        .collect()
}

pub fn format_standings_detail_lines(
    rows: &[StandingRow],
    ranks: &[usize],
    league_slug: &str,
) -> Vec<String> {
    rows.iter()
        .zip(ranks)
        .map(|(row, rank)| format_standing_detail(*rank, row, league_slug))
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

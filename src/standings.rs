use crate::scoring::{DRAW_POINTS, LOSS_POINTS, WIN_POINTS};

/// League-agnostic standings row returned across the `League` seam.
pub struct StandingRow {
    pub user_id: u64,
    pub points: i64,
    pub teams: Vec<(String, i64)>,
    pub tiebreaker_goals: i64,
    pub tiebreaker_player: Option<String>,
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

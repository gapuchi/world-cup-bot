use world_cup_bot::standings::{StandingRow, standings_ranks};

fn standing_row(points: i64) -> StandingRow {
    StandingRow {
        user_id: 0,
        points,
        teams: vec![],
        tiebreaker_goals: 0,
        tiebreaker_player: None,
    }
}

#[test]
fn standings_ranks_tied_points_share_rank() {
    let rows = vec![
        standing_row(3),
        standing_row(3),
        standing_row(0),
        standing_row(0),
        standing_row(0),
        standing_row(0),
    ];

    assert_eq!(standings_ranks(&rows), vec![1, 1, 3, 3, 3, 3]);
}

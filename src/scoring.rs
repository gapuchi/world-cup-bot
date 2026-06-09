pub const WIN_POINTS: i64 = 3;
pub const DRAW_POINTS: i64 = 1;
pub const LOSS_POINTS: i64 = 0;

pub struct FinishedMatch {
    pub home_team_id: i64,
    pub away_team_id: i64,
    pub home_goals: i64,
    pub away_goals: i64,
}

pub fn points_for_result(team_goals: i64, opponent_goals: i64) -> i64 {
    if team_goals > opponent_goals {
        WIN_POINTS
    } else if team_goals == opponent_goals {
        DRAW_POINTS
    } else {
        LOSS_POINTS
    }
}

pub fn points_for_team_in_match(team_id: i64, m: &FinishedMatch) -> i64 {
    if team_id == m.home_team_id {
        points_for_result(m.home_goals, m.away_goals)
    } else if team_id == m.away_team_id {
        points_for_result(m.away_goals, m.home_goals)
    } else {
        0
    }
}

pub fn points_for_team(team_id: i64, matches: &[FinishedMatch]) -> i64 {
    matches
        .iter()
        .map(|m| points_for_team_in_match(team_id, m))
        .sum()
}

pub fn points_for_teams(team_ids: &[i64], matches: &[FinishedMatch]) -> i64 {
    team_ids
        .iter()
        .flat_map(|team_id| {
            matches
                .iter()
                .map(|m| points_for_team_in_match(*team_id, m))
        })
        .sum()
}

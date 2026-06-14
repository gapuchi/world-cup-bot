use crate::db::NFL_LEAGUE_SLUG;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoringRules {
    pub win: i64,
    pub draw: i64,
    pub loss: i64,
}

pub const WC_RULES: ScoringRules = ScoringRules {
    win: 3,
    draw: 1,
    loss: 0,
};

pub const NFL_RULES: ScoringRules = ScoringRules {
    win: 2,
    draw: 1,
    loss: 0,
};

pub fn rules_for_league(slug: &str) -> ScoringRules {
    if slug == NFL_LEAGUE_SLUG {
        NFL_RULES
    } else {
        WC_RULES
    }
}

pub fn format_points(raw: i64, league_slug: &str) -> String {
    if league_slug == NFL_LEAGUE_SLUG {
        if raw % 2 == 0 {
            (raw / 2).to_string()
        } else {
            format!("{}.5", raw / 2)
        }
    } else {
        raw.to_string()
    }
}

pub fn format_rules_footer(rules: &ScoringRules, league_slug: &str) -> String {
    let win = format_points(rules.win, league_slug);
    let draw = format_points(rules.draw, league_slug);
    let loss = format_points(rules.loss, league_slug);
    let tiebreaker = if league_slug == NFL_LEAGUE_SLUG {
        "TB = tie-breaker touchdowns"
    } else {
        "TB = tie-breaker goals"
    };
    format!("Win {win} · Tie {draw} · Loss {loss} · {tiebreaker}")
}

pub struct FinishedMatch {
    pub home_team_id: i64,
    pub away_team_id: i64,
    pub home_goals: i64,
    pub away_goals: i64,
}

pub fn points_for_result(rules: &ScoringRules, team_score: i64, opponent_score: i64) -> i64 {
    if team_score > opponent_score {
        rules.win
    } else if team_score == opponent_score {
        rules.draw
    } else {
        rules.loss
    }
}

pub fn points_for_team_in_match(
    rules: &ScoringRules,
    team_id: i64,
    m: &FinishedMatch,
) -> i64 {
    if team_id == m.home_team_id {
        points_for_result(rules, m.home_goals, m.away_goals)
    } else if team_id == m.away_team_id {
        points_for_result(rules, m.away_goals, m.home_goals)
    } else {
        0
    }
}

pub fn points_for_team(rules: &ScoringRules, team_id: i64, matches: &[FinishedMatch]) -> i64 {
    matches
        .iter()
        .map(|m| points_for_team_in_match(rules, team_id, m))
        .sum()
}

pub fn points_for_teams(rules: &ScoringRules, team_ids: &[i64], matches: &[FinishedMatch]) -> i64 {
    team_ids
        .iter()
        .flat_map(|team_id| {
            matches
                .iter()
                .map(|m| points_for_team_in_match(rules, *team_id, m))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::WC_LEAGUE_SLUG;

    #[test]
    fn nfl_rules_use_half_point_units() {
        let rules = rules_for_league(NFL_LEAGUE_SLUG);
        let m = FinishedMatch {
            home_team_id: 1,
            away_team_id: 2,
            home_goals: 24,
            away_goals: 24,
        };
        assert_eq!(points_for_team_in_match(&rules, 1, &m), 1);
        assert_eq!(format_points(5, NFL_LEAGUE_SLUG), "2.5");
    }

    #[test]
    fn wc_rules_unchanged() {
        let rules = rules_for_league(WC_LEAGUE_SLUG);
        assert_eq!(rules, WC_RULES);
    }
}

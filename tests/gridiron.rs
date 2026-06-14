use world_cup_bot::gridiron::{final_score, find_team, season_date_range};
use world_cup_bot::api::{NflGame, NflTeam};

#[test]
fn final_score_requires_completion_and_both_scores() {
    let incomplete = NflGame {
        id: 1,
        home_team: sample_team(21, "Philadelphia Eagles", "PHI"),
        away_team: sample_team(6, "Dallas Cowboys", "DAL"),
        home_score: Some(24),
        away_score: Some(20),
        completed: false,
    };
    assert_eq!(final_score(&incomplete), None);

    let missing = NflGame {
        completed: true,
        home_score: None,
        away_score: Some(20),
        ..incomplete.clone()
    };
    assert_eq!(final_score(&missing), None);

    let complete = NflGame {
        completed: true,
        home_score: Some(24),
        away_score: Some(20),
        ..incomplete
    };
    assert_eq!(final_score(&complete), Some((24, 20)));
}

#[test]
fn find_team_matches_name_or_abbreviation() {
    let teams = vec![
        sample_team(21, "Philadelphia Eagles", "PHI"),
        sample_team(6, "Dallas Cowboys", "DAL"),
    ];
    assert_eq!(find_team(&teams, "phi").unwrap().id, 21);
    assert_eq!(find_team(&teams, "Cowboys").unwrap().id, 6);
}

#[test]
fn season_date_range_spans_regular_season_and_playoffs() {
    assert_eq!(season_date_range(2025), "20250901-20260215");
}

fn sample_team(id: i64, name: &str, abbreviation: &str) -> NflTeam {
    NflTeam {
        id,
        name: name.into(),
        abbreviation: Some(abbreviation.into()),
    }
}

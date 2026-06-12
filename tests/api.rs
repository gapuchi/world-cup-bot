use world_cup_bot::{
    api::{Match, Score, ScoreDetail, Team},
    soccar::full_time_score,
};

fn sample_match(home: Option<i64>, away: Option<i64>) -> Match {
    Match {
        id: 1,
        home_team: Team {
            id: 769,
            name: "Mexico".into(),
            short_name: None,
            tla: None,
        },
        away_team: Team {
            id: 774,
            name: "South Africa".into(),
            short_name: None,
            tla: None,
        },
        score: Score {
            full_time: ScoreDetail { home, away },
        },
        stage: Some("GROUP_STAGE".into()),
    }
}

#[test]
fn full_time_score_requires_both_sides() {
    assert_eq!(full_time_score(&sample_match(None, None)), None);
    assert_eq!(full_time_score(&sample_match(Some(2), None)), None);
    assert_eq!(full_time_score(&sample_match(None, Some(0))), None);
    assert_eq!(full_time_score(&sample_match(Some(2), Some(0))), Some((2, 0)));
}

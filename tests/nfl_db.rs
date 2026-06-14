use rusqlite::Connection;

use world_cup_bot::db::{
    self, NflMatchResult, NflPlayerTouchdownTotal, NflProcessedGame, NflTiebreakerPick,
};

fn init_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();
    conn
}

fn create_nfl_season(conn: &Connection) -> i64 {
    let season = world_cup_bot::db::Season::get_or_create(
        conn,
        111,
        "nfl",
        "nfl-2025",
        "NFL 2025",
        2025,
    )
    .unwrap();
    season.id
}

#[test]
fn nfl_match_result_upsert_and_list() {
    let conn = init_conn();
    let season_id = create_nfl_season(&conn);

    NflMatchResult {
        season_id,
        game_id: 401772510,
        home_team_id: 21,
        away_team_id: 6,
        home_score: 34,
        away_score: 29,
        finished_at: None,
    }
    .upsert(&conn)
    .unwrap();

    let results = NflMatchResult::list_for_season(&conn, season_id).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(NflMatchResult::score(&conn, season_id, 401772510).unwrap(), Some((34, 29)));
    assert_eq!(results[0].as_finished_match().home_goals, 34);
}

#[test]
fn nfl_processed_game_mark_and_unmark() {
    let conn = init_conn();
    let season_id = create_nfl_season(&conn);

    assert!(!NflProcessedGame::is_processed(&conn, season_id, 99).unwrap());
    NflProcessedGame::mark(&conn, season_id, 99).unwrap();
    assert!(NflProcessedGame::is_processed(&conn, season_id, 99).unwrap());
    NflProcessedGame::unmark(&conn, season_id, 99).unwrap();
    assert!(!NflProcessedGame::is_processed(&conn, season_id, 99).unwrap());
}

#[test]
fn nfl_tiebreaker_pick_round_trip() {
    let conn = init_conn();
    let season_id = create_nfl_season(&conn);

    NflTiebreakerPick::upsert(&conn, season_id, 42, 1234, "Saquon Barkley", 21, "Eagles").unwrap();
    let pick = NflTiebreakerPick::get_for_user(&conn, season_id, 42).unwrap().unwrap();
    assert_eq!(pick.player_name, "Saquon Barkley");
    assert_eq!(pick.team_name, "Eagles");
}

#[test]
fn nfl_player_touchdown_total_upsert_batch() {
    let conn = init_conn();
    let season_id = create_nfl_season(&conn);

    NflPlayerTouchdownTotal::upsert_batch(&conn, season_id, &[(100, 12), (200, 8)], "1").unwrap();
    assert_eq!(
        NflPlayerTouchdownTotal::touchdowns_for_player(&conn, season_id, 100).unwrap(),
        12
    );
    assert_eq!(
        NflPlayerTouchdownTotal::touchdowns_for_player(&conn, season_id, 999).unwrap(),
        0
    );
}

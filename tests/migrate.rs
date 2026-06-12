use rusqlite::Connection;

use world_cup_bot::db;

fn legacy_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
        CREATE TABLE config (id INTEGER PRIMARY KEY CHECK (id = 1), announce_channel_id INTEGER NOT NULL);
        CREATE TABLE registrations (user_id INTEGER NOT NULL, team_id INTEGER PRIMARY KEY NOT NULL, team_name TEXT NOT NULL);
        CREATE TABLE match_results (match_id INTEGER PRIMARY KEY NOT NULL, home_team_id INTEGER NOT NULL, away_team_id INTEGER NOT NULL, home_goals INTEGER NOT NULL, away_goals INTEGER NOT NULL);
        CREATE TABLE processed_matches (match_id INTEGER PRIMARY KEY NOT NULL);
        CREATE TABLE tiebreaker_picks (user_id INTEGER PRIMARY KEY NOT NULL, player_id INTEGER NOT NULL, player_name TEXT NOT NULL, team_id INTEGER NOT NULL, team_name TEXT NOT NULL);
        CREATE TABLE player_goal_totals (player_id INTEGER PRIMARY KEY NOT NULL, goals INTEGER NOT NULL, updated_at TEXT NOT NULL);

        INSERT INTO config VALUES (1, 999888777);
        INSERT INTO registrations VALUES (111, 10, 'Brazil'), (222, 20, 'France');
        INSERT INTO match_results VALUES (1001, 10, 20, 2, 1);
        INSERT INTO processed_matches VALUES (1001);
        INSERT INTO tiebreaker_picks VALUES (111, 501, 'Neymar', 10, 'Brazil');
        INSERT INTO player_goal_totals VALUES (501, 3, '1700000000');
        ",
    )
    .unwrap();
    conn
}

#[test]
fn migrate_legacy_preserves_data() {
    let conn = legacy_db();
    db::init(&conn).unwrap();

    let channel: i64 = conn
        .query_row(
            "SELECT announce_channel_id FROM pools WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(channel, 999888777);

    let registrations: i64 = conn
        .query_row("SELECT COUNT(*) FROM registrations WHERE pool_id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(registrations, 2);

    let goals: i64 = conn
        .query_row(
            "SELECT home_goals FROM wc_match_results WHERE match_id = 1001",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(goals, 2);

    let player_goals: i64 = conn
        .query_row(
            "SELECT goals FROM wc_player_goal_totals WHERE player_id = 501",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(player_goals, 3);

    let legacy_tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name LIKE 'legacy_%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(legacy_tables, 0);

    let league_tables: i64 = conn
        .query_row(
            "
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table'
              AND name IN (
                'nba_match_results',
                'nba_processed_games',
                'nba_tiebreaker_picks',
                'nba_player_points_totals',
                'nfl_match_results',
                'nfl_processed_games',
                'nfl_tiebreaker_picks',
                'nfl_player_touchdown_totals'
              )
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(league_tables, 8);

    let active_pool: i64 = conn
        .query_row(
            "SELECT active_pool_id FROM bot_config WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active_pool, 1);
}

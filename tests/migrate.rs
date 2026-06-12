use rusqlite::Connection;

use world_cup_bot::db::{self, SCHEMA_VERSION};

#[test]
fn fresh_init_seeds_catalog_and_active_pool() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let leagues: i64 = conn
        .query_row("SELECT COUNT(*) FROM leagues", [], |row| row.get(0))
        .unwrap();
    assert_eq!(leagues, 3);

    let wc_pool: i64 = conn
        .query_row(
            "
            SELECT p.id
            FROM pools p
            JOIN seasons s ON s.id = p.season_id
            JOIN leagues l ON l.id = s.league_id
            WHERE l.slug = 'wc' AND s.slug = 'wc-2026'
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();

    let active_pool: i64 = conn
        .query_row(
            "SELECT active_pool_id FROM bot_config WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active_pool, wc_pool);

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
}

#[test]
fn init_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();
    db::init(&conn).unwrap();

    let pools: i64 = conn
        .query_row("SELECT COUNT(*) FROM pools", [], |row| row.get(0))
        .unwrap();
    assert_eq!(pools, 1);
}

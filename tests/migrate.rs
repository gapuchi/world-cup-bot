use rusqlite::Connection;

use world_cup_bot::db::{self, GuildConfig, Pool, Season, SCHEMA_VERSION};

#[test]
fn fresh_init_seeds_catalog_without_pools() {
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

    let seasons: i64 = conn
        .query_row("SELECT COUNT(*) FROM seasons", [], |row| row.get(0))
        .unwrap();
    assert_eq!(seasons, 0);

    let pools: i64 = conn
        .query_row("SELECT COUNT(*) FROM pools", [], |row| row.get(0))
        .unwrap();
    assert_eq!(pools, 0);

    let guild_configs: i64 = conn
        .query_row("SELECT COUNT(*) FROM guild_config", [], |row| row.get(0))
        .unwrap();
    assert_eq!(guild_configs, 0);

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
    assert_eq!(pools, 0);
}

#[test]
fn get_or_create_for_season_scopes_by_guild() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let season = Season::get_or_create(&conn, "wc", "wc-2026", "World Cup 2026").unwrap();

    let guild_a = 111_u64;
    let guild_b = 222_u64;

    let pool_a = Pool::get_or_create_for_season(&conn, guild_a, season.id).unwrap();
    let pool_b = Pool::get_or_create_for_season(&conn, guild_b, season.id).unwrap();

    assert_ne!(pool_a.id, pool_b.id);
    assert_eq!(pool_a.guild_id, guild_a);
    assert_eq!(pool_b.guild_id, guild_b);

    GuildConfig::set_default_pool_id(&conn, guild_a, pool_a.id).unwrap();
    GuildConfig::set_default_pool_id(&conn, guild_b, pool_b.id).unwrap();

    assert_eq!(Pool::default_for_guild(&conn, guild_a).unwrap().id, pool_a.id);
    assert_eq!(Pool::default_for_guild(&conn, guild_b).unwrap().id, pool_b.id);
}

#[test]
fn fresh_init_has_no_external_season_id_column() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let has_column: bool = conn
        .prepare("PRAGMA table_info(seasons)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .flatten()
        .any(|name| name == "external_season_id");
    assert!(!has_column);
}

#[test]
fn migration_v2_drops_external_season_id() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
        CREATE TABLE schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version (version) VALUES (1);
        CREATE TABLE leagues (
            id INTEGER PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            sport TEXT NOT NULL
        );
        CREATE TABLE seasons (
            id INTEGER PRIMARY KEY,
            league_id INTEGER NOT NULL REFERENCES leagues(id),
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            external_season_id TEXT,
            UNIQUE (league_id, slug)
        );
        ",
    )
    .unwrap();

    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let has_column: bool = conn
        .prepare("PRAGMA table_info(seasons)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .flatten()
        .any(|name| name == "external_season_id");
    assert!(!has_column);
}

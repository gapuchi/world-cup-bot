use rusqlite::Connection;

use world_cup_bot::db::{self, GuildConfig, Pool, SCHEMA_VERSION};

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
fn get_or_create_for_league_scopes_by_guild() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_a = 111_u64;
    let guild_b = 222_u64;

    let pool_a = Pool::get_or_create_for_league(&conn, guild_a, "wc").unwrap();
    let pool_b = Pool::get_or_create_for_league(&conn, guild_b, "wc").unwrap();

    assert_ne!(pool_a.id, pool_b.id);
    assert_eq!(pool_a.guild_id, guild_a);
    assert_eq!(pool_b.guild_id, guild_b);

    GuildConfig::set_default_pool_id(&conn, guild_a, pool_a.id).unwrap();
    GuildConfig::set_default_pool_id(&conn, guild_b, pool_b.id).unwrap();

    assert_eq!(Pool::default_for_guild(&conn, guild_a).unwrap().id, pool_a.id);
    assert_eq!(Pool::default_for_guild(&conn, guild_b).unwrap().id, pool_b.id);
}

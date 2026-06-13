use rusqlite::Connection;

use world_cup_bot::db::{self, GuildConfig, LEGACY_GUILD_ID, Pool, SCHEMA_VERSION};

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
fn v1_migration_assigns_legacy_guild() {
    let conn = Connection::open_in_memory().unwrap();
    seed_v1_database(&conn);

    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let guild_id: i64 = conn
        .query_row("SELECT guild_id FROM pools WHERE id = 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(guild_id, LEGACY_GUILD_ID);

    let default_pool: i64 = conn
        .query_row(
            "SELECT default_pool_id FROM guild_config WHERE guild_id = ?1",
            [LEGACY_GUILD_ID],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(default_pool, 1);

    let bot_config_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'bot_config'",
            [],
            |_| Ok(()),
        )
        .is_ok();
    assert!(!bot_config_exists);

    let registration: (i64, i64) = conn
        .query_row(
            "SELECT user_id, team_id FROM registrations WHERE pool_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(registration, (42, 77));

    let pool = Pool::default_for_guild(&conn, LEGACY_GUILD_ID as u64).unwrap();
    assert_eq!(pool.id, 1);
    assert_eq!(pool.guild_id, LEGACY_GUILD_ID as u64);

    let config = GuildConfig::get(&conn, LEGACY_GUILD_ID as u64).unwrap().unwrap();
    assert_eq!(config.default_pool_id, 1);
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

fn seed_v1_database(conn: &Connection) {
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
        INSERT INTO leagues (id, slug, name, sport)
        VALUES (1, 'wc', 'FIFA World Cup', 'soccer');

        CREATE TABLE seasons (
            id INTEGER PRIMARY KEY,
            league_id INTEGER NOT NULL REFERENCES leagues(id),
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            external_season_id TEXT,
            starts_at TEXT,
            ends_at TEXT,
            UNIQUE (league_id, slug)
        );
        INSERT INTO seasons (id, league_id, slug, name, external_season_id)
        VALUES (1, 1, 'wc-2026', 'World Cup 2026', 'WC');

        CREATE TABLE pools (
            id INTEGER PRIMARY KEY,
            season_id INTEGER NOT NULL REFERENCES seasons(id),
            announce_channel_id INTEGER,
            UNIQUE (season_id)
        );
        INSERT INTO pools (id, season_id, announce_channel_id) VALUES (1, 1, 999);

        CREATE TABLE bot_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            active_pool_id INTEGER NOT NULL REFERENCES pools(id)
        );
        INSERT INTO bot_config (id, active_pool_id) VALUES (1, 1);

        CREATE TABLE registrations (
            pool_id INTEGER NOT NULL REFERENCES pools(id),
            user_id INTEGER NOT NULL,
            team_id INTEGER NOT NULL,
            team_name TEXT NOT NULL,
            PRIMARY KEY (pool_id, team_id)
        );
        INSERT INTO registrations (pool_id, user_id, team_id, team_name)
        VALUES (1, 42, 77, 'Brazil');
        ",
    )
    .unwrap();
}

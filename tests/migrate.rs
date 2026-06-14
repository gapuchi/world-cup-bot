use rusqlite::Connection;

use world_cup_bot::db::{self, GuildConfig, Season, SCHEMA_VERSION};

#[test]
fn fresh_init_seeds_catalog_without_seasons() {
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

    let seasons: i64 = conn
        .query_row("SELECT COUNT(*) FROM seasons", [], |row| row.get(0))
        .unwrap();
    assert_eq!(seasons, 0);
}

#[test]
fn get_or_create_scopes_by_guild() {
    let conn = Connection::open_in_memory().unwrap();
    db::init(&conn).unwrap();

    let guild_a = 111_u64;
    let guild_b = 222_u64;

    let season_a =
        Season::get_or_create(&conn, guild_a, "wc", "wc-2026", "World Cup 2026", 2026).unwrap();
    let season_b =
        Season::get_or_create(&conn, guild_b, "wc", "wc-2026", "World Cup 2026", 2026).unwrap();

    assert_ne!(season_a.id, season_b.id);
    assert_eq!(season_a.guild_id, guild_a);
    assert_eq!(season_b.guild_id, guild_b);

    GuildConfig::set_default_season_id(&conn, guild_a, season_a.id).unwrap();
    GuildConfig::set_default_season_id(&conn, guild_b, season_b.id).unwrap();

    assert_eq!(
        Season::default_for_guild(&conn, guild_a).unwrap().id,
        season_a.id
    );
    assert_eq!(
        Season::default_for_guild(&conn, guild_b).unwrap().id,
        season_b.id
    );
}

#[test]
fn migration_v3_merges_pool_into_season() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
        CREATE TABLE schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version (version) VALUES (2);
        CREATE TABLE leagues (
            id INTEGER PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            sport TEXT NOT NULL
        );
        INSERT INTO leagues (id, slug, name, sport) VALUES (1, 'wc', 'FIFA World Cup', 'soccer');
        CREATE TABLE seasons (
            id INTEGER PRIMARY KEY,
            league_id INTEGER NOT NULL REFERENCES leagues(id),
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            UNIQUE (league_id, slug)
        );
        INSERT INTO seasons (id, league_id, slug, name) VALUES (10, 1, 'wc-2026', 'World Cup 2026');
        CREATE TABLE pools (
            id INTEGER PRIMARY KEY,
            guild_id INTEGER NOT NULL,
            season_id INTEGER NOT NULL REFERENCES seasons(id),
            announce_channel_id INTEGER,
            UNIQUE (guild_id, season_id)
        );
        INSERT INTO pools (id, guild_id, season_id, announce_channel_id) VALUES (1, 111, 10, 999);
        INSERT INTO pools (id, guild_id, season_id) VALUES (2, 222, 10);
        CREATE TABLE guild_config (
            guild_id INTEGER PRIMARY KEY,
            default_pool_id INTEGER NOT NULL REFERENCES pools(id)
        );
        INSERT INTO guild_config (guild_id, default_pool_id) VALUES (111, 1);
        CREATE TABLE registrations (
            pool_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            team_id INTEGER NOT NULL,
            team_name TEXT NOT NULL,
            PRIMARY KEY (pool_id, team_id)
        );
        INSERT INTO registrations (pool_id, user_id, team_id, team_name) VALUES (1, 42, 7, 'Brazil');
        CREATE TABLE wc_match_results (
            pool_id INTEGER NOT NULL,
            match_id INTEGER NOT NULL,
            home_team_id INTEGER NOT NULL,
            away_team_id INTEGER NOT NULL,
            home_goals INTEGER NOT NULL,
            away_goals INTEGER NOT NULL,
            stage TEXT,
            finished_at TEXT,
            PRIMARY KEY (pool_id, match_id)
        );
        CREATE TABLE wc_processed_matches (
            pool_id INTEGER NOT NULL,
            match_id INTEGER NOT NULL,
            PRIMARY KEY (pool_id, match_id)
        );
        CREATE TABLE wc_tiebreaker_picks (
            pool_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            player_id INTEGER NOT NULL,
            player_name TEXT NOT NULL,
            team_id INTEGER NOT NULL,
            team_name TEXT NOT NULL,
            PRIMARY KEY (pool_id, user_id)
        );
        CREATE TABLE wc_player_goal_totals (
            season_id INTEGER NOT NULL,
            player_id INTEGER NOT NULL,
            goals INTEGER NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (season_id, player_id)
        );
        INSERT INTO wc_player_goal_totals (season_id, player_id, goals, updated_at)
        VALUES (10, 99, 3, '1');
        ",
    )
    .unwrap();

    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let has_pools: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'pools'",
            [],
            |_| Ok(()),
        )
        .is_ok();
    assert!(!has_pools);

    let season = Season::get(&conn, 1).unwrap().unwrap();
    assert_eq!(season.guild_id, 111);
    assert_eq!(season.slug, "wc-2026");
    assert_eq!(season.announce_channel_id, Some(999));

    let default_season_id: i64 = conn
        .query_row(
            "SELECT default_season_id FROM guild_config WHERE guild_id = 111",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(default_season_id, 1);

    let team_name: String = conn
        .query_row(
            "SELECT team_name FROM registrations WHERE season_id = 1 AND team_id = 7",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(team_name, "Brazil");

    let goals_for_pool_1: i64 = conn
        .query_row(
            "SELECT goals FROM wc_player_goal_totals WHERE season_id = 1 AND player_id = 99",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let goals_for_pool_2: i64 = conn
        .query_row(
            "SELECT goals FROM wc_player_goal_totals WHERE season_id = 2 AND player_id = 99",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(goals_for_pool_1, 3);
    assert_eq!(goals_for_pool_2, 3);
}

#[test]
fn migration_v3_merges_legacy_single_guild_pool() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
        CREATE TABLE schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version (version) VALUES (2);
        CREATE TABLE leagues (
            id INTEGER PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            sport TEXT NOT NULL
        );
        INSERT INTO leagues (id, slug, name, sport) VALUES (1, 'wc', 'FIFA World Cup', 'soccer');
        CREATE TABLE seasons (
            id INTEGER PRIMARY KEY,
            league_id INTEGER NOT NULL REFERENCES leagues(id),
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            external_season_id TEXT,
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
        CREATE TABLE registrations (
            pool_id INTEGER NOT NULL REFERENCES pools(id),
            user_id INTEGER NOT NULL,
            team_id INTEGER NOT NULL,
            team_name TEXT NOT NULL,
            PRIMARY KEY (pool_id, team_id)
        );
        INSERT INTO registrations (pool_id, user_id, team_id, team_name) VALUES (1, 42, 7, 'Brazil');
        ",
    )
    .unwrap();

    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let season = Season::get(&conn, 1).unwrap().unwrap();
    assert_eq!(season.guild_id, 527_288_150_515_646_484);
    assert_eq!(season.announce_channel_id, Some(999));

    let default_season_id: i64 = conn
        .query_row(
            "SELECT default_season_id FROM guild_config WHERE guild_id = ?1",
            [527_288_150_515_646_484_i64],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(default_season_id, 1);

    let team_name: String = conn
        .query_row(
            "SELECT team_name FROM registrations WHERE season_id = 1 AND team_id = 7",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(team_name, "Brazil");
}

#[test]
fn migration_v4_rebinds_foreign_keys_after_pool_table_drop() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        CREATE TABLE schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version (version) VALUES (3);
        CREATE TABLE leagues (
            id INTEGER PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            sport TEXT NOT NULL
        );
        INSERT INTO leagues (id, slug, name, sport) VALUES (1, 'wc', 'FIFA World Cup', 'soccer');
        CREATE TABLE seasons (
            id INTEGER PRIMARY KEY,
            guild_id INTEGER NOT NULL,
            league_id INTEGER NOT NULL REFERENCES leagues(id),
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            announce_channel_id INTEGER,
            UNIQUE (guild_id, league_id, slug)
        );
        INSERT INTO seasons (id, guild_id, league_id, slug, name, announce_channel_id)
        VALUES (1, 111, 1, 'wc-2026', 'World Cup 2026', 999);
        CREATE TABLE pools (id INTEGER PRIMARY KEY);
        INSERT INTO pools (id) VALUES (1);
        CREATE TABLE wc_match_results (
            pool_id INTEGER NOT NULL REFERENCES pools(id),
            match_id INTEGER NOT NULL,
            home_team_id INTEGER NOT NULL,
            away_team_id INTEGER NOT NULL,
            home_goals INTEGER NOT NULL,
            away_goals INTEGER NOT NULL,
            stage TEXT,
            finished_at TEXT,
            PRIMARY KEY (pool_id, match_id)
        );
        ALTER TABLE wc_match_results RENAME COLUMN pool_id TO season_id;
        DROP TABLE pools;
        ",
    )
    .unwrap();

    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let referenced_table: String = conn
        .query_row(
            "SELECT [table] FROM pragma_foreign_key_list('wc_match_results') LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(referenced_table, "seasons");

    use world_cup_bot::db::WcMatchResult;
    WcMatchResult {
        season_id: 1,
        match_id: 42,
        home_team_id: 10,
        away_team_id: 11,
        home_goals: 2,
        away_goals: 1,
        stage: Some("Group".into()),
    }
    .upsert(&conn)
    .unwrap();
}

#[test]
fn migration_v5_adds_draft_tables() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
        CREATE TABLE schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version (version) VALUES (4);
        CREATE TABLE leagues (
            id INTEGER PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            sport TEXT NOT NULL
        );
        INSERT INTO leagues (id, slug, name, sport) VALUES (1, 'wc', 'FIFA World Cup', 'soccer');
        CREATE TABLE seasons (
            id INTEGER PRIMARY KEY,
            guild_id INTEGER NOT NULL,
            league_id INTEGER NOT NULL REFERENCES leagues(id),
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            announce_channel_id INTEGER,
            starts_at TEXT,
            ends_at TEXT,
            UNIQUE (guild_id, league_id, slug)
        );
        INSERT INTO seasons (id, guild_id, league_id, slug, name)
        VALUES (1, 111, 1, 'wc-2026', 'World Cup 2026');
        CREATE TABLE guild_config (
            guild_id INTEGER PRIMARY KEY,
            default_season_id INTEGER NOT NULL REFERENCES seasons(id)
        );
        INSERT INTO guild_config (guild_id, default_season_id) VALUES (111, 1);
        CREATE TABLE registrations (
            season_id INTEGER NOT NULL REFERENCES seasons(id),
            user_id INTEGER NOT NULL,
            team_id INTEGER NOT NULL,
            team_name TEXT NOT NULL,
            PRIMARY KEY (season_id, team_id)
        );
        ",
    )
    .unwrap();

    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let draft_tables: i64 = conn
        .query_row(
            "
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table'
              AND name IN ('drafts', 'draft_participants')
            ",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(draft_tables, 2);

    use world_cup_bot::db::Draft;
    Draft::create_active(&conn, 1, 2, 6, &[(10, 0), (20, 1), (30, 2)]).unwrap();
    assert_eq!(Draft::current_picker(&conn, 1).unwrap(), Some(10));
}

#[test]
fn migration_v6_adds_season_year() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
        CREATE TABLE schema_version (version INTEGER NOT NULL);
        INSERT INTO schema_version (version) VALUES (5);
        CREATE TABLE leagues (
            id INTEGER PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            sport TEXT NOT NULL
        );
        INSERT INTO leagues (id, slug, name, sport) VALUES (1, 'wc', 'FIFA World Cup', 'soccer');
        CREATE TABLE seasons (
            id INTEGER PRIMARY KEY,
            guild_id INTEGER NOT NULL,
            league_id INTEGER NOT NULL REFERENCES leagues(id),
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            announce_channel_id INTEGER,
            starts_at TEXT,
            ends_at TEXT,
            UNIQUE (guild_id, league_id, slug)
        );
        INSERT INTO seasons (id, guild_id, league_id, slug, name)
        VALUES (1, 111, 1, 'wc-2026', 'World Cup 2026');
        CREATE TABLE guild_config (
            guild_id INTEGER PRIMARY KEY,
            default_season_id INTEGER NOT NULL REFERENCES seasons(id)
        );
        INSERT INTO guild_config (guild_id, default_season_id) VALUES (111, 1);
        ",
    )
    .unwrap();

    db::init(&conn).unwrap();

    let version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let season_year: i64 = conn
        .query_row("SELECT season_year FROM seasons WHERE id = 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(season_year, 2026);
}

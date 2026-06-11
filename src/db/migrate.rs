use rusqlite::{Connection, OptionalExtension, Transaction};

pub const SCHEMA_VERSION: i64 = 2;
pub const WC_LEAGUE_SLUG: &str = "wc";
pub const WC_SEASON_SLUG: &str = "wc-2026";
pub const NBA_LEAGUE_SLUG: &str = "nba";
pub const NBA_SEASON_SLUG: &str = "nba-2025-26";
pub const NFL_LEAGUE_SLUG: &str = "nfl";
pub const NFL_SEASON_SLUG: &str = "nfl-2025";

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS leagues (
    id                      INTEGER PRIMARY KEY,
    slug                    TEXT NOT NULL UNIQUE,
    name                    TEXT NOT NULL,
    sport                   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS seasons (
    id                      INTEGER PRIMARY KEY,
    league_id               INTEGER NOT NULL REFERENCES leagues(id),
    slug                    TEXT NOT NULL UNIQUE,
    name                    TEXT NOT NULL,
    external_season_id      TEXT,
    starts_at               TEXT,
    ends_at                 TEXT,
    UNIQUE (league_id, slug)
);

CREATE TABLE IF NOT EXISTS pools (
    id                      INTEGER PRIMARY KEY,
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    announce_channel_id     INTEGER,
    UNIQUE (season_id)
);

CREATE TABLE IF NOT EXISTS teams (
    league_id               INTEGER NOT NULL,
    team_id                 INTEGER NOT NULL,
    name                    TEXT NOT NULL,
    short_name              TEXT,
    code                    TEXT,
    PRIMARY KEY (league_id, team_id),
    FOREIGN KEY (league_id) REFERENCES leagues(id)
);

CREATE TABLE IF NOT EXISTS registrations (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    user_id                 INTEGER NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (pool_id, team_id)
);

CREATE INDEX IF NOT EXISTS idx_registrations_pool_user
    ON registrations (pool_id, user_id);

CREATE TABLE IF NOT EXISTS wc_match_results (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    match_id                INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_goals              INTEGER NOT NULL,
    away_goals              INTEGER NOT NULL,
    stage                   TEXT,
    finished_at             TEXT,
    PRIMARY KEY (pool_id, match_id)
);

CREATE TABLE IF NOT EXISTS wc_processed_matches (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    match_id                INTEGER NOT NULL,
    PRIMARY KEY (pool_id, match_id)
);

CREATE TABLE IF NOT EXISTS wc_tiebreaker_picks (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (pool_id, user_id)
);

CREATE TABLE IF NOT EXISTS wc_player_goal_totals (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    player_id               INTEGER NOT NULL,
    goals                   INTEGER NOT NULL,
    updated_at              TEXT NOT NULL,
    PRIMARY KEY (season_id, player_id)
);

CREATE TABLE IF NOT EXISTS nba_match_results (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    game_id                 INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_points             INTEGER NOT NULL,
    away_points             INTEGER NOT NULL,
    finished_at             TEXT,
    PRIMARY KEY (pool_id, game_id)
);

CREATE TABLE IF NOT EXISTS nba_processed_games (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    game_id                 INTEGER NOT NULL,
    PRIMARY KEY (pool_id, game_id)
);

CREATE TABLE IF NOT EXISTS nba_tiebreaker_picks (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (pool_id, user_id)
);

CREATE TABLE IF NOT EXISTS nba_player_points_totals (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    player_id               INTEGER NOT NULL,
    points                  INTEGER NOT NULL,
    updated_at              TEXT NOT NULL,
    PRIMARY KEY (season_id, player_id)
);

CREATE TABLE IF NOT EXISTS nfl_match_results (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    game_id                 INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_score              INTEGER NOT NULL,
    away_score              INTEGER NOT NULL,
    finished_at             TEXT,
    PRIMARY KEY (pool_id, game_id)
);

CREATE TABLE IF NOT EXISTS nfl_processed_games (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    game_id                 INTEGER NOT NULL,
    PRIMARY KEY (pool_id, game_id)
);

CREATE TABLE IF NOT EXISTS nfl_tiebreaker_picks (
    pool_id                 INTEGER NOT NULL REFERENCES pools(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (pool_id, user_id)
);

CREATE TABLE IF NOT EXISTS nfl_player_touchdown_totals (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    player_id               INTEGER NOT NULL,
    touchdowns              INTEGER NOT NULL,
    updated_at              TEXT NOT NULL,
    PRIMARY KEY (season_id, player_id)
);
";

pub fn run(conn: &Connection) -> rusqlite::Result<()> {
    let version = current_version(conn)?;

    if version.is_none() {
        let legacy = has_legacy_schema(conn)?;
        if legacy {
            rename_legacy_tables(conn)?;
        }
        conn.execute_batch(CREATE_SCHEMA)?;
        seed_catalog(conn)?;
        let tx = conn.unchecked_transaction()?;
        if legacy {
            migrate_legacy(&tx)?;
        }
        set_version(&tx, SCHEMA_VERSION)?;
        tx.commit()?;
        return Ok(());
    }

    conn.execute_batch(CREATE_SCHEMA)?;
    seed_catalog(conn)?;

    if version == Some(1) {
        migrate_v1_to_v2(conn)?;
    }

    Ok(())
}

fn current_version(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    if !table_exists(conn, "schema_version")? {
        return Ok(None);
    }
    conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .optional()
}

fn set_version(tx: &Transaction, version: i64) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM schema_version", [])?;
    tx.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
    Ok(())
}

fn seed_catalog(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO leagues (id, slug, name, sport)
        VALUES (1, ?1, 'FIFA World Cup', 'soccer')
        ON CONFLICT(id) DO NOTHING
        ",
        [WC_LEAGUE_SLUG],
    )?;
    conn.execute(
        "
        INSERT INTO seasons (id, league_id, slug, name, external_season_id)
        VALUES (1, 1, ?1, 'World Cup 2026', 'WC')
        ON CONFLICT(id) DO NOTHING
        ",
        [WC_SEASON_SLUG],
    )?;
    conn.execute(
        "
        INSERT INTO leagues (id, slug, name, sport)
        VALUES (2, ?1, 'NBA', 'basketball')
        ON CONFLICT(id) DO NOTHING
        ",
        [NBA_LEAGUE_SLUG],
    )?;
    conn.execute(
        "
        INSERT INTO leagues (id, slug, name, sport)
        VALUES (3, ?1, 'NFL', 'football')
        ON CONFLICT(id) DO NOTHING
        ",
        [NFL_LEAGUE_SLUG],
    )?;
    conn.execute(
        "
        INSERT INTO seasons (id, league_id, slug, name, external_season_id)
        VALUES (2, 2, ?1, 'NBA 2025–26', 'NBA')
        ON CONFLICT(id) DO NOTHING
        ",
        [NBA_SEASON_SLUG],
    )?;
    conn.execute(
        "
        INSERT INTO seasons (id, league_id, slug, name, external_season_id)
        VALUES (3, 3, ?1, 'NFL 2025', 'NFL')
        ON CONFLICT(id) DO NOTHING
        ",
        [NFL_SEASON_SLUG],
    )?;
    Ok(())
}

fn table_exists(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .map(|row| row.is_some())
}

fn has_legacy_schema(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(table_exists(conn, "config")?
        && table_exists(conn, "registrations")?
        && table_exists(conn, "match_results")?)
}

fn rename_legacy_tables(conn: &Connection) -> rusqlite::Result<()> {
    for (old, new) in [
        ("config", "legacy_config"),
        ("registrations", "legacy_registrations"),
        ("match_results", "legacy_match_results"),
        ("processed_matches", "legacy_processed_matches"),
        ("tiebreaker_picks", "legacy_tiebreaker_picks"),
        ("player_goal_totals", "legacy_player_goal_totals"),
    ] {
        if table_exists(conn, old)? {
            conn.execute(&format!("ALTER TABLE {old} RENAME TO {new}"), [])?;
        }
    }
    Ok(())
}

fn migrate_legacy(tx: &Transaction) -> rusqlite::Result<()> {
    let announce_channel_id: Option<i64> = tx
        .query_row(
            "SELECT announce_channel_id FROM legacy_config WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()?;

    tx.execute(
        "
        INSERT INTO pools (id, season_id, announce_channel_id)
        VALUES (1, 1, ?1)
        ",
        rusqlite::params![announce_channel_id],
    )?;

    tx.execute(
        "
        INSERT OR IGNORE INTO teams (league_id, team_id, name)
        SELECT 1, team_id, team_name
        FROM legacy_registrations
        ",
        [],
    )?;

    tx.execute(
        "
        INSERT INTO registrations (pool_id, user_id, team_id, team_name)
        SELECT 1, user_id, team_id, team_name
        FROM legacy_registrations
        ",
        [],
    )?;

    tx.execute(
        "
        INSERT INTO wc_match_results (
            pool_id, match_id, home_team_id, away_team_id, home_goals, away_goals
        )
        SELECT 1, match_id, home_team_id, away_team_id, home_goals, away_goals
        FROM legacy_match_results
        ",
        [],
    )?;

    tx.execute(
        "
        INSERT INTO wc_processed_matches (pool_id, match_id)
        SELECT 1, match_id
        FROM legacy_processed_matches
        ",
        [],
    )?;

    tx.execute(
        "
        INSERT INTO wc_tiebreaker_picks (
            pool_id, user_id, player_id, player_name, team_id, team_name
        )
        SELECT 1, user_id, player_id, player_name, team_id, team_name
        FROM legacy_tiebreaker_picks
        ",
        [],
    )?;

    tx.execute(
        "
        INSERT INTO wc_player_goal_totals (season_id, player_id, goals, updated_at)
        SELECT 1, player_id, goals, updated_at
        FROM legacy_player_goal_totals
        ",
        [],
    )?;

    drop_legacy_tables(tx)
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows.flatten().any(|name| name == column))
}

fn migrate_v1_to_v2(conn: &Connection) -> rusqlite::Result<()> {
    if !column_exists(conn, "pools", "guild_id")? {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    tx.execute("PRAGMA foreign_keys = OFF", [])?;
    tx.execute_batch(
        "
        CREATE TABLE pools_new (
            id                      INTEGER PRIMARY KEY,
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            announce_channel_id     INTEGER,
            UNIQUE (season_id)
        );
        INSERT INTO pools_new (id, season_id, announce_channel_id)
        SELECT id, season_id, announce_channel_id FROM pools;
        DROP TABLE pools;
        ALTER TABLE pools_new RENAME TO pools;
        ",
    )?;
    tx.execute("PRAGMA foreign_keys = ON", [])?;
    set_version(&tx, SCHEMA_VERSION)?;
    tx.commit()
}

fn drop_legacy_tables(tx: &Transaction) -> rusqlite::Result<()> {
    for table in [
        "legacy_player_goal_totals",
        "legacy_tiebreaker_picks",
        "legacy_processed_matches",
        "legacy_match_results",
        "legacy_registrations",
        "legacy_config",
    ] {
        tx.execute(&format!("DROP TABLE IF EXISTS {table}"), [])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::run;

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
        run(&conn).unwrap();

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
    }
}

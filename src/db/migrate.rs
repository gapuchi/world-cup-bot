use rusqlite::{Connection, OptionalExtension, Transaction};

pub const SCHEMA_VERSION: i64 = 3;
pub const WC_LEAGUE_SLUG: &str = "wc";
pub const NBA_LEAGUE_SLUG: &str = "nba";
pub const NFL_LEAGUE_SLUG: &str = "nfl";

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
    guild_id                INTEGER NOT NULL,
    league_id               INTEGER NOT NULL REFERENCES leagues(id),
    slug                    TEXT NOT NULL,
    name                    TEXT NOT NULL,
    announce_channel_id     INTEGER,
    starts_at               TEXT,
    ends_at                 TEXT,
    UNIQUE (guild_id, league_id, slug)
);

CREATE TABLE IF NOT EXISTS guild_config (
    guild_id                INTEGER PRIMARY KEY,
    default_season_id       INTEGER NOT NULL REFERENCES seasons(id)
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
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    user_id                 INTEGER NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (season_id, team_id)
);

CREATE INDEX IF NOT EXISTS idx_registrations_season_user
    ON registrations (season_id, user_id);

CREATE TABLE IF NOT EXISTS wc_match_results (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    match_id                INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_goals              INTEGER NOT NULL,
    away_goals              INTEGER NOT NULL,
    stage                   TEXT,
    finished_at             TEXT,
    PRIMARY KEY (season_id, match_id)
);

CREATE TABLE IF NOT EXISTS wc_processed_matches (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    match_id                INTEGER NOT NULL,
    PRIMARY KEY (season_id, match_id)
);

CREATE TABLE IF NOT EXISTS wc_tiebreaker_picks (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (season_id, user_id)
);

CREATE TABLE IF NOT EXISTS wc_player_goal_totals (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    player_id               INTEGER NOT NULL,
    goals                   INTEGER NOT NULL,
    updated_at              TEXT NOT NULL,
    PRIMARY KEY (season_id, player_id)
);

CREATE TABLE IF NOT EXISTS nba_match_results (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_points             INTEGER NOT NULL,
    away_points             INTEGER NOT NULL,
    finished_at             TEXT,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS nba_processed_games (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS nba_tiebreaker_picks (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (season_id, user_id)
);

CREATE TABLE IF NOT EXISTS nba_player_points_totals (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    player_id               INTEGER NOT NULL,
    points                  INTEGER NOT NULL,
    updated_at              TEXT NOT NULL,
    PRIMARY KEY (season_id, player_id)
);

CREATE TABLE IF NOT EXISTS nfl_match_results (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    home_team_id            INTEGER NOT NULL,
    away_team_id            INTEGER NOT NULL,
    home_score              INTEGER NOT NULL,
    away_score              INTEGER NOT NULL,
    finished_at             TEXT,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS nfl_processed_games (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    game_id                 INTEGER NOT NULL,
    PRIMARY KEY (season_id, game_id)
);

CREATE TABLE IF NOT EXISTS nfl_tiebreaker_picks (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    user_id                 INTEGER NOT NULL,
    player_id               INTEGER NOT NULL,
    player_name             TEXT NOT NULL,
    team_id                 INTEGER NOT NULL,
    team_name               TEXT NOT NULL,
    PRIMARY KEY (season_id, user_id)
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
    let version = current_version(conn)?.unwrap_or(0);

    if version == 0 {
        conn.execute_batch(CREATE_SCHEMA)?;
        seed_catalog(conn)?;
        set_version(conn, SCHEMA_VERSION)?;
        return Ok(());
    }

    seed_catalog(conn)?;

    if version < 2 {
        migrate_v2_drop_external_season_id(conn)?;
    }

    if version < 3 {
        migrate_v3_merge_season_and_pool(conn)?;
    }

    conn.execute_batch(CREATE_SCHEMA)?;

    if version < SCHEMA_VERSION {
        set_version(conn, SCHEMA_VERSION)?;
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

fn set_version(conn: &Connection, version: i64) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    set_version_tx(&tx, version)?;
    tx.commit()?;
    Ok(())
}

fn set_version_tx(tx: &Transaction<'_>, version: i64) -> rusqlite::Result<()> {
    if !table_exists(tx, "schema_version")? {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);",
        )?;
    }
    tx.execute("DELETE FROM schema_version", [])?;
    tx.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
    Ok(())
}

fn migrate_v2_drop_external_season_id(conn: &Connection) -> rusqlite::Result<()> {
    if table_exists(conn, "seasons")?
        && column_exists(conn, "seasons", "external_season_id")?
    {
        conn.execute("ALTER TABLE seasons DROP COLUMN external_season_id", [])?;
    }
    Ok(())
}

fn migrate_v3_merge_season_and_pool(conn: &Connection) -> rusqlite::Result<()> {
    if !table_exists(conn, "pools")? {
        return Ok(());
    }

    conn.execute("PRAGMA foreign_keys = OFF", [])?;
    let tx = conn.unchecked_transaction()?;

    tx.execute("ALTER TABLE seasons RENAME TO seasons_legacy", [])?;

    tx.execute_batch(
        "
        CREATE TABLE seasons (
            id                      INTEGER PRIMARY KEY,
            guild_id                INTEGER NOT NULL,
            league_id               INTEGER NOT NULL REFERENCES leagues(id),
            slug                    TEXT NOT NULL,
            name                    TEXT NOT NULL,
            announce_channel_id     INTEGER,
            starts_at               TEXT,
            ends_at                 TEXT,
            UNIQUE (guild_id, league_id, slug)
        );
        ",
    )?;

    tx.execute(
        "
        INSERT INTO seasons (id, guild_id, league_id, slug, name, announce_channel_id)
        SELECT p.id, p.guild_id, s.league_id, s.slug, s.name, p.announce_channel_id
        FROM pools p
        JOIN seasons_legacy s ON s.id = p.season_id
        ",
        [],
    )?;

    tx.execute(
        "ALTER TABLE guild_config RENAME COLUMN default_pool_id TO default_season_id",
        [],
    )?;

    for table in [
        "registrations",
        "wc_match_results",
        "wc_processed_matches",
        "wc_tiebreaker_picks",
        "nba_match_results",
        "nba_processed_games",
        "nba_tiebreaker_picks",
        "nfl_match_results",
        "nfl_processed_games",
        "nfl_tiebreaker_picks",
    ] {
        if table_exists(&tx, table)? && column_exists(&tx, table, "pool_id")? {
            tx.execute(
                &format!("ALTER TABLE {table} RENAME COLUMN pool_id TO season_id"),
                [],
            )?;
        }
    }

    if table_exists(&tx, "wc_player_goal_totals")? {
        migrate_player_totals_to_per_season(&tx, "wc_player_goal_totals", "goals")?;
    }
    if table_exists(&tx, "nba_player_points_totals")? {
        migrate_player_totals_to_per_season(&tx, "nba_player_points_totals", "points")?;
    }
    if table_exists(&tx, "nfl_player_touchdown_totals")? {
        migrate_player_totals_to_per_season(&tx, "nfl_player_touchdown_totals", "touchdowns")?;
    }

    tx.execute("DROP INDEX IF EXISTS idx_registrations_pool_user", [])?;
    tx.execute(
        "
        CREATE INDEX IF NOT EXISTS idx_registrations_season_user
            ON registrations (season_id, user_id)
        ",
        [],
    )?;

    tx.execute("DROP TABLE pools", [])?;
    tx.execute("DROP TABLE seasons_legacy", [])?;

    tx.execute_batch(
        "
        CREATE TABLE guild_config_new (
            guild_id                INTEGER PRIMARY KEY,
            default_season_id       INTEGER NOT NULL REFERENCES seasons(id)
        );
        INSERT INTO guild_config_new (guild_id, default_season_id)
        SELECT guild_id, default_season_id FROM guild_config;
        DROP TABLE guild_config;
        ALTER TABLE guild_config_new RENAME TO guild_config;
        ",
    )?;

    tx.commit()?;
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    Ok(())
}

fn migrate_player_totals_to_per_season(
    tx: &Transaction<'_>,
    table: &str,
    stat_column: &str,
) -> rusqlite::Result<()> {
    let temp = format!("{table}_migrated");
    tx.execute_batch(&format!(
        "
        CREATE TABLE {temp} (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            player_id               INTEGER NOT NULL,
            {stat_column}           INTEGER NOT NULL,
            updated_at              TEXT NOT NULL,
            PRIMARY KEY (season_id, player_id)
        );
        INSERT INTO {temp} (season_id, player_id, {stat_column}, updated_at)
        SELECT p.id, t.player_id, t.{stat_column}, t.updated_at
        FROM {table} t
        JOIN pools p ON p.season_id = t.season_id;
        DROP TABLE {table};
        ALTER TABLE {temp} RENAME TO {table};
        "
    ))?;
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in rows.flatten() {
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
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

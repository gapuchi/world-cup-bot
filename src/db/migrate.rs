use rusqlite::{Connection, OptionalExtension, Transaction};

pub const SCHEMA_VERSION: i64 = 1;
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
    guild_id                INTEGER NOT NULL,
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    announce_channel_id     INTEGER,
    UNIQUE (guild_id, season_id)
);

CREATE TABLE IF NOT EXISTS guild_config (
    guild_id                INTEGER PRIMARY KEY,
    default_pool_id         INTEGER NOT NULL REFERENCES pools(id)
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

    conn.execute_batch(CREATE_SCHEMA)?;
    seed_catalog(conn)?;

    if version.is_none() {
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

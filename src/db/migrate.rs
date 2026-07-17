use rusqlite::{Connection, OptionalExtension, Transaction};

pub const SCHEMA_VERSION: i64 = 6;
pub const WC_LEAGUE_SLUG: &str = "wc";
pub const NBA_LEAGUE_SLUG: &str = "nba";
pub const NFL_LEAGUE_SLUG: &str = "nfl";

/// Single-guild installs created before multi-guild support did not store guild id on pools.
const LEGACY_GUILD_ID: i64 = 527_288_150_515_646_484;

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
    polling_enabled         INTEGER NOT NULL DEFAULT 1,
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

CREATE TABLE IF NOT EXISTS wc_announced_eliminations (
    season_id               INTEGER NOT NULL REFERENCES seasons(id),
    team_id                 INTEGER NOT NULL,
    PRIMARY KEY (season_id, team_id)
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

    if version < 4 {
        migrate_v4_rebind_season_foreign_keys(conn)?;
    }

    if version < 5 {
        migrate_v5_add_announced_eliminations(conn)?;
    }

    if version < 6 {
        migrate_v6_add_season_polling_enabled(conn)?;
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
        if tables_still_use_pool_id(conn)? {
            return Err(rusqlite::Error::InvalidParameterName(
                "pools table is missing but gameplay tables still reference pool_id; restore from backup or recreate the database".into(),
            ));
        }
        return Ok(());
    }

    conn.execute("PRAGMA foreign_keys = OFF", [])?;
    let tx = conn.unchecked_transaction()?;

    let legacy_guild_id = legacy_guild_id_for_migration(&tx)?;
    if !column_exists(&tx, "pools", "guild_id")? {
        upgrade_legacy_pools(&tx, legacy_guild_id)?;
    }
    migrate_bot_config_to_guild_config(&tx, legacy_guild_id)?;

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

    if table_exists(&tx, "guild_config")?
        && column_exists(&tx, "guild_config", "default_pool_id")?
    {
        tx.execute(
            "ALTER TABLE guild_config RENAME COLUMN default_pool_id TO default_season_id",
            [],
        )?;
    }

    rebind_pool_scoped_tables(&tx)?;

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

    if table_exists(&tx, "guild_config")? {
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
    } else {
        tx.execute_batch(
            "
            CREATE TABLE guild_config (
                guild_id                INTEGER PRIMARY KEY,
                default_season_id       INTEGER NOT NULL REFERENCES seasons(id)
            );
            INSERT INTO guild_config (guild_id, default_season_id)
            SELECT guild_id, MIN(id) FROM seasons GROUP BY guild_id;
            ",
        )?;
    }

    tx.commit()?;
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    Ok(())
}

fn migrate_v4_rebind_season_foreign_keys(conn: &Connection) -> rusqlite::Result<()> {
    if !tables_reference_pools(conn)? {
        return Ok(());
    }

    conn.execute("PRAGMA foreign_keys = OFF", [])?;
    let tx = conn.unchecked_transaction()?;
    rebind_pool_scoped_tables(&tx)?;
    tx.execute("DROP INDEX IF EXISTS idx_registrations_pool_user", [])?;
    if table_exists(&tx, "registrations")? {
        tx.execute(
            "
            CREATE INDEX IF NOT EXISTS idx_registrations_season_user
                ON registrations (season_id, user_id)
            ",
            [],
        )?;
    }
    tx.commit()?;
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    Ok(())
}

fn tables_reference_pools(conn: &Connection) -> rusqlite::Result<bool> {
    for table in pool_scoped_tables() {
        if foreign_key_references_table(conn, table, "pools")? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn pool_scoped_tables() -> &'static [&'static str] {
    &[
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
    ]
}

fn foreign_key_references_table(
    conn: &Connection,
    table: &str,
    referenced: &str,
) -> rusqlite::Result<bool> {
    if !table_exists(conn, table)? {
        return Ok(false);
    }
    let sql = format!("PRAGMA foreign_key_list({table})");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(2))?;
    for ref_table in rows.flatten() {
        if ref_table == referenced {
            return Ok(true);
        }
    }
    Ok(false)
}

fn scope_column(conn: &Connection, table: &str) -> rusqlite::Result<&'static str> {
    if column_exists(conn, table, "pool_id")? {
        Ok("pool_id")
    } else {
        Ok("season_id")
    }
}

fn needs_pool_scope_rebind(conn: &Connection, table: &str) -> rusqlite::Result<bool> {
    Ok(column_exists(conn, table, "pool_id")?
        || foreign_key_references_table(conn, table, "pools")?)
}

fn rebind_pool_scoped_tables(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    rebind_registrations(tx)?;
    rebind_wc_match_results(tx)?;
    rebind_wc_processed_matches(tx)?;
    rebind_wc_tiebreaker_picks(tx)?;
    rebind_nba_match_results(tx)?;
    rebind_nba_processed_games(tx)?;
    rebind_nba_tiebreaker_picks(tx)?;
    rebind_nfl_match_results(tx)?;
    rebind_nfl_processed_games(tx)?;
    rebind_nfl_tiebreaker_picks(tx)?;
    Ok(())
}

fn rebind_registrations(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "registrations")? {
        return Ok(());
    }
    let scope = scope_column(tx, "registrations")?;
    tx.execute_batch(
        "
        CREATE TABLE registrations_season_fk (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            user_id                 INTEGER NOT NULL,
            team_id                 INTEGER NOT NULL,
            team_name               TEXT NOT NULL,
            PRIMARY KEY (season_id, team_id)
        );
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO registrations_season_fk (season_id, user_id, team_id, team_name)
            SELECT {scope}, user_id, team_id, team_name FROM registrations
            "
        ),
        [],
    )?;
    tx.execute_batch("DROP TABLE registrations; ALTER TABLE registrations_season_fk RENAME TO registrations;")?;
    Ok(())
}

fn rebind_wc_match_results(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "wc_match_results")? {
        return Ok(());
    }
    let scope = scope_column(tx, "wc_match_results")?;
    tx.execute_batch(
        "
        CREATE TABLE wc_match_results_season_fk (
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
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO wc_match_results_season_fk (
                season_id, match_id, home_team_id, away_team_id, home_goals, away_goals, stage, finished_at
            )
            SELECT {scope}, match_id, home_team_id, away_team_id, home_goals, away_goals, stage, finished_at
            FROM wc_match_results
            "
        ),
        [],
    )?;
    tx.execute_batch(
        "DROP TABLE wc_match_results; ALTER TABLE wc_match_results_season_fk RENAME TO wc_match_results;",
    )?;
    Ok(())
}

fn rebind_wc_processed_matches(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "wc_processed_matches")? {
        return Ok(());
    }
    let scope = scope_column(tx, "wc_processed_matches")?;
    tx.execute_batch(
        "
        CREATE TABLE wc_processed_matches_season_fk (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            match_id                INTEGER NOT NULL,
            PRIMARY KEY (season_id, match_id)
        );
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO wc_processed_matches_season_fk (season_id, match_id)
            SELECT {scope}, match_id FROM wc_processed_matches
            "
        ),
        [],
    )?;
    tx.execute_batch(
        "DROP TABLE wc_processed_matches; ALTER TABLE wc_processed_matches_season_fk RENAME TO wc_processed_matches;",
    )?;
    Ok(())
}

fn rebind_wc_tiebreaker_picks(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "wc_tiebreaker_picks")? {
        return Ok(());
    }
    let scope = scope_column(tx, "wc_tiebreaker_picks")?;
    tx.execute_batch(
        "
        CREATE TABLE wc_tiebreaker_picks_season_fk (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            user_id                 INTEGER NOT NULL,
            player_id               INTEGER NOT NULL,
            player_name             TEXT NOT NULL,
            team_id                 INTEGER NOT NULL,
            team_name               TEXT NOT NULL,
            PRIMARY KEY (season_id, user_id)
        );
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO wc_tiebreaker_picks_season_fk (
                season_id, user_id, player_id, player_name, team_id, team_name
            )
            SELECT {scope}, user_id, player_id, player_name, team_id, team_name
            FROM wc_tiebreaker_picks
            "
        ),
        [],
    )?;
    tx.execute_batch(
        "DROP TABLE wc_tiebreaker_picks; ALTER TABLE wc_tiebreaker_picks_season_fk RENAME TO wc_tiebreaker_picks;",
    )?;
    Ok(())
}

fn rebind_nba_match_results(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "nba_match_results")? {
        return Ok(());
    }
    let scope = scope_column(tx, "nba_match_results")?;
    tx.execute_batch(
        "
        CREATE TABLE nba_match_results_season_fk (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            game_id                 INTEGER NOT NULL,
            home_team_id            INTEGER NOT NULL,
            away_team_id            INTEGER NOT NULL,
            home_points             INTEGER NOT NULL,
            away_points             INTEGER NOT NULL,
            finished_at             TEXT,
            PRIMARY KEY (season_id, game_id)
        );
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO nba_match_results_season_fk (
                season_id, game_id, home_team_id, away_team_id, home_points, away_points, finished_at
            )
            SELECT {scope}, game_id, home_team_id, away_team_id, home_points, away_points, finished_at
            FROM nba_match_results
            "
        ),
        [],
    )?;
    tx.execute_batch(
        "DROP TABLE nba_match_results; ALTER TABLE nba_match_results_season_fk RENAME TO nba_match_results;",
    )?;
    Ok(())
}

fn rebind_nba_processed_games(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "nba_processed_games")? {
        return Ok(());
    }
    let scope = scope_column(tx, "nba_processed_games")?;
    tx.execute_batch(
        "
        CREATE TABLE nba_processed_games_season_fk (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            game_id                 INTEGER NOT NULL,
            PRIMARY KEY (season_id, game_id)
        );
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO nba_processed_games_season_fk (season_id, game_id)
            SELECT {scope}, game_id FROM nba_processed_games
            "
        ),
        [],
    )?;
    tx.execute_batch(
        "DROP TABLE nba_processed_games; ALTER TABLE nba_processed_games_season_fk RENAME TO nba_processed_games;",
    )?;
    Ok(())
}

fn rebind_nba_tiebreaker_picks(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "nba_tiebreaker_picks")? {
        return Ok(());
    }
    let scope = scope_column(tx, "nba_tiebreaker_picks")?;
    tx.execute_batch(
        "
        CREATE TABLE nba_tiebreaker_picks_season_fk (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            user_id                 INTEGER NOT NULL,
            player_id               INTEGER NOT NULL,
            player_name             TEXT NOT NULL,
            team_id                 INTEGER NOT NULL,
            team_name               TEXT NOT NULL,
            PRIMARY KEY (season_id, user_id)
        );
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO nba_tiebreaker_picks_season_fk (
                season_id, user_id, player_id, player_name, team_id, team_name
            )
            SELECT {scope}, user_id, player_id, player_name, team_id, team_name
            FROM nba_tiebreaker_picks
            "
        ),
        [],
    )?;
    tx.execute_batch(
        "DROP TABLE nba_tiebreaker_picks; ALTER TABLE nba_tiebreaker_picks_season_fk RENAME TO nba_tiebreaker_picks;",
    )?;
    Ok(())
}

fn rebind_nfl_match_results(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "nfl_match_results")? {
        return Ok(());
    }
    let scope = scope_column(tx, "nfl_match_results")?;
    tx.execute_batch(
        "
        CREATE TABLE nfl_match_results_season_fk (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            game_id                 INTEGER NOT NULL,
            home_team_id            INTEGER NOT NULL,
            away_team_id            INTEGER NOT NULL,
            home_score              INTEGER NOT NULL,
            away_score              INTEGER NOT NULL,
            finished_at             TEXT,
            PRIMARY KEY (season_id, game_id)
        );
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO nfl_match_results_season_fk (
                season_id, game_id, home_team_id, away_team_id, home_score, away_score, finished_at
            )
            SELECT {scope}, game_id, home_team_id, away_team_id, home_score, away_score, finished_at
            FROM nfl_match_results
            "
        ),
        [],
    )?;
    tx.execute_batch(
        "DROP TABLE nfl_match_results; ALTER TABLE nfl_match_results_season_fk RENAME TO nfl_match_results;",
    )?;
    Ok(())
}

fn rebind_nfl_processed_games(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "nfl_processed_games")? {
        return Ok(());
    }
    let scope = scope_column(tx, "nfl_processed_games")?;
    tx.execute_batch(
        "
        CREATE TABLE nfl_processed_games_season_fk (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            game_id                 INTEGER NOT NULL,
            PRIMARY KEY (season_id, game_id)
        );
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO nfl_processed_games_season_fk (season_id, game_id)
            SELECT {scope}, game_id FROM nfl_processed_games
            "
        ),
        [],
    )?;
    tx.execute_batch(
        "DROP TABLE nfl_processed_games; ALTER TABLE nfl_processed_games_season_fk RENAME TO nfl_processed_games;",
    )?;
    Ok(())
}

fn rebind_nfl_tiebreaker_picks(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if !needs_pool_scope_rebind(tx, "nfl_tiebreaker_picks")? {
        return Ok(());
    }
    let scope = scope_column(tx, "nfl_tiebreaker_picks")?;
    tx.execute_batch(
        "
        CREATE TABLE nfl_tiebreaker_picks_season_fk (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            user_id                 INTEGER NOT NULL,
            player_id               INTEGER NOT NULL,
            player_name             TEXT NOT NULL,
            team_id                 INTEGER NOT NULL,
            team_name               TEXT NOT NULL,
            PRIMARY KEY (season_id, user_id)
        );
        ",
    )?;
    tx.execute(
        &format!(
            "
            INSERT INTO nfl_tiebreaker_picks_season_fk (
                season_id, user_id, player_id, player_name, team_id, team_name
            )
            SELECT {scope}, user_id, player_id, player_name, team_id, team_name
            FROM nfl_tiebreaker_picks
            "
        ),
        [],
    )?;
    tx.execute_batch(
        "DROP TABLE nfl_tiebreaker_picks; ALTER TABLE nfl_tiebreaker_picks_season_fk RENAME TO nfl_tiebreaker_picks;",
    )?;
    Ok(())
}

fn tables_still_use_pool_id(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(table_exists(conn, "registrations")?
        && column_exists(conn, "registrations", "pool_id")?)
}

fn legacy_guild_id_for_migration(_conn: &Connection) -> rusqlite::Result<i64> {
    if let Ok(raw) = std::env::var("LEGACY_GUILD_ID") {
        return raw.parse::<i64>().map_err(|_| {
            rusqlite::Error::InvalidParameterName(format!(
                "LEGACY_GUILD_ID must be a Discord guild id, got {raw:?}"
            ))
        });
    }
    Ok(LEGACY_GUILD_ID)
}

fn upgrade_legacy_pools(tx: &Transaction<'_>, guild_id: i64) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
        CREATE TABLE pools_new (
            id                      INTEGER PRIMARY KEY,
            guild_id                INTEGER NOT NULL,
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            announce_channel_id     INTEGER,
            UNIQUE (guild_id, season_id)
        );
        ",
    )?;
    tx.execute(
        "
        INSERT INTO pools_new (id, guild_id, season_id, announce_channel_id)
        SELECT id, ?1, season_id, announce_channel_id FROM pools
        ",
        [guild_id],
    )?;
    tx.execute("DROP TABLE pools", [])?;
    tx.execute("ALTER TABLE pools_new RENAME TO pools", [])?;
    Ok(())
}

fn migrate_bot_config_to_guild_config(
    tx: &Transaction<'_>,
    guild_id: i64,
) -> rusqlite::Result<()> {
    if !table_exists(tx, "bot_config")? || table_exists(tx, "guild_config")? {
        return Ok(());
    }

    tx.execute_batch(
        "
        CREATE TABLE guild_config (
            guild_id                INTEGER PRIMARY KEY,
            default_pool_id         INTEGER NOT NULL REFERENCES pools(id)
        );
        ",
    )?;
    tx.execute(
        "
        INSERT INTO guild_config (guild_id, default_pool_id)
        SELECT ?1, active_pool_id FROM bot_config WHERE id = 1
        ",
        [guild_id],
    )?;
    tx.execute("DROP TABLE bot_config", [])?;
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

fn migrate_v5_add_announced_eliminations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS wc_announced_eliminations (
            season_id               INTEGER NOT NULL REFERENCES seasons(id),
            team_id                 INTEGER NOT NULL,
            PRIMARY KEY (season_id, team_id)
        );
        ",
    )?;
    Ok(())
}

fn migrate_v6_add_season_polling_enabled(conn: &Connection) -> rusqlite::Result<()> {
    if table_exists(conn, "seasons")? && !column_exists(conn, "seasons", "polling_enabled")? {
        conn.execute(
            "ALTER TABLE seasons ADD COLUMN polling_enabled INTEGER NOT NULL DEFAULT 1",
            [],
        )?;
    }
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

use rusqlite::{Connection, OptionalExtension, params};

use super::{
    guild_config::GuildConfig,
    season::Season,
};

pub struct Pool {
    pub id: i64,
    pub guild_id: u64,
    pub season_id: i64,
    pub announce_channel_id: Option<u64>,
}

pub struct PoolMeta {
    pub pool: Pool,
    pub season_id: i64,
    pub external_season_id: String,
    pub league_slug: String,
    pub league_name: String,
}

pub struct PoolLeague {
    pub pool: Pool,
    pub league_slug: String,
    pub league_name: String,
}

impl Pool {
    pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "SELECT id, guild_id, season_id, announce_channel_id FROM pools WHERE id = ?1",
            params![id],
            row_from,
        )
        .optional()
    }

    pub fn default_for_guild(conn: &Connection, guild_id: u64) -> rusqlite::Result<Self> {
        let config = GuildConfig::get(conn, guild_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let pool = Self::get(conn, config.default_pool_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        if pool.guild_id != guild_id {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(pool)
    }

    pub fn list_all_with_meta(conn: &Connection) -> rusqlite::Result<Vec<PoolMeta>> {
        let mut stmt = conn.prepare(
            "
            SELECT
                p.id,
                p.guild_id,
                p.season_id,
                p.announce_channel_id,
                s.id,
                s.external_season_id,
                l.slug,
                l.name
            FROM pools p
            JOIN seasons s ON s.id = p.season_id
            JOIN leagues l ON l.id = s.league_id
            ORDER BY p.id
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let external: Option<String> = row.get(5)?;
            Ok(PoolMeta {
                pool: pool_from_row(row, 0, 1, 2, 3)?,
                season_id: row.get(4)?,
                external_season_id: external.unwrap_or_default(),
                league_slug: row.get(6)?,
                league_name: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_or_create_for_league(
        conn: &Connection,
        guild_id: u64,
        league_slug: &str,
    ) -> rusqlite::Result<Self> {
        let season = Season::get_for_league(conn, league_slug)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;

        if let Some(id) = pool_id_for_guild_season(conn, guild_id, season.id)? {
            return Self::get(conn, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows);
        }

        conn.execute(
            "INSERT INTO pools (guild_id, season_id) VALUES (?1, ?2)",
            params![guild_id as i64, season.id],
        )?;
        Self::get(conn, conn.last_insert_rowid())?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn set_announce_channel(
        conn: &Connection,
        id: i64,
        channel_id: u64,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "UPDATE pools SET announce_channel_id = ?1 WHERE id = ?2",
            params![channel_id as i64, id],
        )?;
        Ok(())
    }

    pub fn list_with_league(conn: &Connection, guild_id: u64) -> rusqlite::Result<Vec<PoolLeague>> {
        let mut stmt = conn.prepare(
            "
            SELECT p.id, p.guild_id, p.season_id, p.announce_channel_id, l.slug, l.name
            FROM pools p
            JOIN seasons s ON s.id = p.season_id
            JOIN leagues l ON l.id = s.league_id
            WHERE p.guild_id = ?1
            ORDER BY l.id
            ",
        )?;
        let rows = stmt.query_map(params![guild_id as i64], |row| {
            Ok(PoolLeague {
                pool: pool_from_row(row, 0, 1, 2, 3)?,
                league_slug: row.get(4)?,
                league_name: row.get(5)?,
            })
        })?;
        rows.collect()
    }
}

fn pool_id_for_guild_season(
    conn: &Connection,
    guild_id: u64,
    season_id: i64,
) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM pools WHERE guild_id = ?1 AND season_id = ?2",
        params![guild_id as i64, season_id],
        |row| row.get(0),
    )
    .optional()
}

fn pool_from_row(
    row: &rusqlite::Row<'_>,
    id_col: usize,
    guild_col: usize,
    season_col: usize,
    channel_col: usize,
) -> rusqlite::Result<Pool> {
    let channel: Option<i64> = row.get(channel_col)?;
    Ok(Pool {
        id: row.get(id_col)?,
        guild_id: row.get::<_, i64>(guild_col)? as u64,
        season_id: row.get(season_col)?,
        announce_channel_id: channel.map(|id| id as u64),
    })
}

fn row_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<Pool> {
    pool_from_row(row, 0, 1, 2, 3)
}

use rusqlite::{Connection, OptionalExtension, params};

use super::{
    bot_config::BotConfig,
    season::Season,
};

pub struct Pool {
    pub id: i64,
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
            "SELECT id, season_id, announce_channel_id FROM pools WHERE id = ?1",
            params![id],
            |row| row_from(row),
        )
        .optional()
    }

    pub fn active(conn: &Connection) -> rusqlite::Result<Self> {
        let config = BotConfig::get(conn)?.ok_or_else(|| {
            rusqlite::Error::QueryReturnedNoRows
        })?;
        Self::get(conn, config.active_pool_id)?.ok_or_else(|| {
            rusqlite::Error::QueryReturnedNoRows
        })
    }

    pub fn list_all_with_meta(conn: &Connection) -> rusqlite::Result<Vec<PoolMeta>> {
        let mut stmt = conn.prepare(
            "
            SELECT
                p.id,
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
            let channel: Option<i64> = row.get(2)?;
            let external: Option<String> = row.get(4)?;
            Ok(PoolMeta {
                pool: Pool {
                    id: row.get(0)?,
                    season_id: row.get(1)?,
                    announce_channel_id: channel.map(|id| id as u64),
                },
                season_id: row.get(3)?,
                external_season_id: external.unwrap_or_default(),
                league_slug: row.get(5)?,
                league_name: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_or_create_for_league(
        conn: &Connection,
        league_slug: &str,
    ) -> rusqlite::Result<Self> {
        let season = Season::get_for_league(conn, league_slug)?.ok_or_else(|| {
            rusqlite::Error::QueryReturnedNoRows
        })?;

        if let Some(id) = pool_id_for_season(conn, season.id)? {
            return Self::get(conn, id)?.ok_or_else(|| {
                rusqlite::Error::QueryReturnedNoRows
            });
        }

        conn.execute(
            "INSERT INTO pools (season_id) VALUES (?1)",
            params![season.id],
        )?;
        Self::get(conn, conn.last_insert_rowid())?.ok_or_else(|| {
            rusqlite::Error::QueryReturnedNoRows
        })
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

    pub fn list_with_league(conn: &Connection) -> rusqlite::Result<Vec<PoolLeague>> {
        let mut stmt = conn.prepare(
            "
            SELECT p.id, p.season_id, p.announce_channel_id, l.slug, l.name
            FROM pools p
            JOIN seasons s ON s.id = p.season_id
            JOIN leagues l ON l.id = s.league_id
            ORDER BY l.id
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let channel: Option<i64> = row.get(2)?;
            Ok(PoolLeague {
                pool: Pool {
                    id: row.get(0)?,
                    season_id: row.get(1)?,
                    announce_channel_id: channel.map(|id| id as u64),
                },
                league_slug: row.get(3)?,
                league_name: row.get(4)?,
            })
        })?;
        rows.collect()
    }
}

fn pool_id_for_season(conn: &Connection, season_id: i64) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM pools WHERE season_id = ?1",
        params![season_id],
        |row| row.get(0),
    )
    .optional()
}

fn row_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<Pool> {
    let channel: Option<i64> = row.get(2)?;
    Ok(Pool {
        id: row.get(0)?,
        season_id: row.get(1)?,
        announce_channel_id: channel.map(|id| id as u64),
    })
}

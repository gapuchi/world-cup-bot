use rusqlite::{Connection, OptionalExtension, params};

use super::migrate::{WC_LEAGUE_SLUG, WC_SEASON_SLUG};

pub struct Pool {
    pub id: i64,
    pub season_id: i64,
    pub announce_channel_id: Option<u64>,
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

    pub fn ensure_wc(conn: &Connection) -> rusqlite::Result<Self> {
        if let Some(id) = wc_pool_id(conn)? {
            return Self::get(conn, id)?.ok_or_else(|| {
                rusqlite::Error::QueryReturnedNoRows
            });
        }

        let season_id: i64 = conn.query_row(
            "
            SELECT s.id
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            WHERE l.slug = ?1 AND s.slug = ?2
            ",
            params![WC_LEAGUE_SLUG, WC_SEASON_SLUG],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT INTO pools (season_id) VALUES (?1)",
            params![season_id],
        )?;
        Self::get(conn, conn.last_insert_rowid())?.ok_or_else(|| {
            rusqlite::Error::QueryReturnedNoRows
        })
    }

    pub fn list_wc(conn: &Connection) -> rusqlite::Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "
            SELECT p.id, p.season_id, p.announce_channel_id
            FROM pools p
            JOIN seasons s ON s.id = p.season_id
            JOIN leagues l ON l.id = s.league_id
            WHERE l.slug = ?1 AND s.slug = ?2
            ORDER BY p.id
            ",
        )?;
        let rows = stmt.query_map(params![WC_LEAGUE_SLUG, WC_SEASON_SLUG], row_from)?;
        rows.collect()
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
}

fn wc_pool_id(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "
        SELECT p.id
        FROM pools p
        JOIN seasons s ON s.id = p.season_id
        JOIN leagues l ON l.id = s.league_id
        WHERE l.slug = ?1 AND s.slug = ?2
        ",
        params![WC_LEAGUE_SLUG, WC_SEASON_SLUG],
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

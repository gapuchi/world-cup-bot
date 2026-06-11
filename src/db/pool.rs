use rusqlite::{Connection, OptionalExtension, params};

use super::migrate::{WC_LEAGUE_SLUG, WC_SEASON_SLUG};

pub struct Pool {
    pub id: i64,
    pub announce_channel_id: Option<u64>,
}

pub fn ensure_wc_pool(conn: &Connection) -> rusqlite::Result<i64> {
    if let Some(pool_id) = wc_pool_id(conn)? {
        return Ok(pool_id);
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
    Ok(conn.last_insert_rowid())
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

pub fn list_wc_pools(conn: &Connection) -> rusqlite::Result<Vec<Pool>> {
    let mut stmt = conn.prepare(
        "
        SELECT p.id, p.announce_channel_id
        FROM pools p
        JOIN seasons s ON s.id = p.season_id
        JOIN leagues l ON l.id = s.league_id
        WHERE l.slug = ?1 AND s.slug = ?2
        ORDER BY p.id
        ",
    )?;
    let rows = stmt.query_map(params![WC_LEAGUE_SLUG, WC_SEASON_SLUG], |row| {
        let channel: Option<i64> = row.get(1)?;
        Ok(Pool {
            id: row.get(0)?,
            announce_channel_id: channel.map(|id| id as u64),
        })
    })?;
    rows.collect()
}

pub fn set_announce_channel(
    conn: &Connection,
    pool_id: i64,
    channel_id: u64,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE pools SET announce_channel_id = ?1 WHERE id = ?2",
        params![channel_id as i64, pool_id],
    )?;
    Ok(())
}

use rusqlite::{Connection, OptionalExtension, params};

use super::league;

pub struct Season {
    pub id: i64,
}

impl Season {
    pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "SELECT id FROM seasons WHERE id = ?1",
            params![id],
            |row| Ok(Self { id: row.get(0)? }),
        )
        .optional()
    }

    pub fn get_by_slug(conn: &Connection, slug: &str) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "SELECT id FROM seasons WHERE slug = ?1",
            params![slug],
            |row| Ok(Self { id: row.get(0)? }),
        )
        .optional()
    }

    pub fn get_or_create(
        conn: &Connection,
        league_slug: &str,
        slug: &str,
        name: &str,
    ) -> rusqlite::Result<Self> {
        if let Some(season) = Self::get_by_slug(conn, slug)? {
            return Ok(season);
        }

        let league_id = league::id_for_slug(conn, league_slug)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;

        conn.execute(
            "INSERT INTO seasons (league_id, slug, name) VALUES (?1, ?2, ?3)",
            params![league_id, slug, name],
        )?;
        Self::get(conn, conn.last_insert_rowid())?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn league_id_for_pool(conn: &Connection, pool_id: i64) -> rusqlite::Result<i64> {
        conn.query_row(
            "
            SELECT s.league_id
            FROM pools p
            JOIN seasons s ON s.id = p.season_id
            WHERE p.id = ?1
            ",
            params![pool_id],
            |row| row.get(0),
        )
    }

    pub fn league_slug_for_pool(conn: &Connection, pool_id: i64) -> rusqlite::Result<String> {
        conn.query_row(
            "
            SELECT l.slug
            FROM pools p
            JOIN seasons s ON s.id = p.season_id
            JOIN leagues l ON l.id = s.league_id
            WHERE p.id = ?1
            ",
            params![pool_id],
            |row| row.get(0),
        )
    }
}

pub struct SeasonDisplay {
    pub league_name: String,
    pub name: String,
    pub slug: String,
}

impl SeasonDisplay {
    pub fn for_pool(conn: &Connection, pool_id: i64) -> rusqlite::Result<Self> {
        conn.query_row(
            "
            SELECT l.name, s.name, s.slug
            FROM pools p
            JOIN seasons s ON s.id = p.season_id
            JOIN leagues l ON l.id = s.league_id
            WHERE p.id = ?1
            ",
            params![pool_id],
            |row| {
                Ok(Self {
                    league_name: row.get(0)?,
                    name: row.get(1)?,
                    slug: row.get(2)?,
                })
            },
        )
    }
}

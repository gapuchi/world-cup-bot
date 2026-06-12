use rusqlite::{Connection, OptionalExtension, params};

pub struct Season {
    pub id: i64,
    pub external_season_id: Option<String>,
}

impl Season {
    pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "SELECT id, external_season_id FROM seasons WHERE id = ?1",
            params![id],
            |row| {
                Ok(Self {
                    id: row.get(0)?,
                    external_season_id: row.get(1)?,
                })
            },
        )
        .optional()
    }

    pub fn get_for_league(conn: &Connection, league_slug: &str) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "
            SELECT s.id, s.external_season_id
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            WHERE l.slug = ?1
            ORDER BY s.id
            LIMIT 1
            ",
            params![league_slug],
            |row| {
                Ok(Self {
                    id: row.get(0)?,
                    external_season_id: row.get(1)?,
                })
            },
        )
        .optional()
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

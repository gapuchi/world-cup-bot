use rusqlite::{Connection, params};

use super::migrate::{WC_LEAGUE_SLUG, WC_SEASON_SLUG};

pub struct SeasonDisplay {
    pub league_name: String,
    pub name: String,
    pub slug: String,
}

impl SeasonDisplay {
    pub fn wc(conn: &Connection) -> rusqlite::Result<Self> {
        conn.query_row(
            "
            SELECT l.name, s.name, s.slug
            FROM seasons s
            JOIN leagues l ON l.id = s.league_id
            WHERE l.slug = ?1 AND s.slug = ?2
            ",
            params![WC_LEAGUE_SLUG, WC_SEASON_SLUG],
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

pub fn wc_season_id(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "
        SELECT s.id
        FROM seasons s
        JOIN leagues l ON l.id = s.league_id
        WHERE l.slug = ?1 AND s.slug = ?2
        ",
        params![WC_LEAGUE_SLUG, WC_SEASON_SLUG],
        |row| row.get(0),
    )
}

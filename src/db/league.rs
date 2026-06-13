use rusqlite::{Connection, OptionalExtension, params};

use super::migrate::WC_LEAGUE_SLUG;

pub fn id_for_slug(conn: &Connection, slug: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM leagues WHERE slug = ?1",
        params![slug],
        |row| row.get(0),
    )
    .optional()
}

pub fn exists(conn: &Connection, slug: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT 1 FROM leagues WHERE slug = ?1",
        params![slug],
        |_| Ok(()),
    )
    .optional()
    .map(|row| row.is_some())
}

pub fn supports_pool(slug: &str) -> bool {
    slug == WC_LEAGUE_SLUG
}

pub fn competition_code(slug: &str) -> String {
    slug.to_uppercase()
}

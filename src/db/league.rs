use rusqlite::{Connection, OptionalExtension, params};

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

pub fn competition_code(slug: &str) -> String {
    match slug {
        "epl" => "PL".into(),
        slug => slug.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::competition_code;

    #[test]
    fn competition_code_maps_league_slugs() {
        assert_eq!(competition_code("wc"), "WC");
        assert_eq!(competition_code("epl"), "PL");
    }
}

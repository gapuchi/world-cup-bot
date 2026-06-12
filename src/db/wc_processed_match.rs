use rusqlite::{Connection, OptionalExtension, params};

pub struct WcProcessedMatch {
    pub pool_id: i64,
    pub match_id: i64,
}

impl WcProcessedMatch {
    pub fn is_processed(
        conn: &Connection,
        pool_id: i64,
        match_id: i64,
    ) -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT 1 FROM wc_processed_matches WHERE pool_id = ?1 AND match_id = ?2",
            params![pool_id, match_id],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
    }

    pub fn mark(conn: &Connection, pool_id: i64, match_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT OR IGNORE INTO wc_processed_matches (pool_id, match_id)
            VALUES (?1, ?2)
            ",
            params![pool_id, match_id],
        )?;
        Ok(())
    }

    pub fn unmark(conn: &Connection, pool_id: i64, match_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "DELETE FROM wc_processed_matches WHERE pool_id = ?1 AND match_id = ?2",
            params![pool_id, match_id],
        )?;
        Ok(())
    }
}

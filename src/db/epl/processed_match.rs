use rusqlite::{Connection, OptionalExtension, params};

pub struct EplProcessedMatch;

impl EplProcessedMatch {
    pub fn is_processed(
        conn: &Connection,
        season_id: i64,
        match_id: i64,
    ) -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT 1 FROM epl_processed_matches WHERE season_id = ?1 AND match_id = ?2",
            params![season_id, match_id],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
    }

    pub fn mark(conn: &Connection, season_id: i64, match_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT OR IGNORE INTO epl_processed_matches (season_id, match_id)
            VALUES (?1, ?2)
            ",
            params![season_id, match_id],
        )?;
        Ok(())
    }

    pub fn unmark(conn: &Connection, season_id: i64, match_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "DELETE FROM epl_processed_matches WHERE season_id = ?1 AND match_id = ?2",
            params![season_id, match_id],
        )?;
        Ok(())
    }
}

use std::collections::HashSet;

use rusqlite::{Connection, params};

pub struct WcAnnouncedElimination;

impl WcAnnouncedElimination {
    pub fn list_for_season(conn: &Connection, season_id: i64) -> rusqlite::Result<HashSet<i64>> {
        let mut stmt = conn.prepare(
            "
            SELECT team_id
            FROM wc_announced_eliminations
            WHERE season_id = ?1
            ",
        )?;
        let rows = stmt.query_map(params![season_id], |row| row.get(0))?;
        rows.collect()
    }

    pub fn mark(conn: &Connection, season_id: i64, team_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT OR IGNORE INTO wc_announced_eliminations (season_id, team_id)
            VALUES (?1, ?2)
            ",
            params![season_id, team_id],
        )?;
        Ok(())
    }
}

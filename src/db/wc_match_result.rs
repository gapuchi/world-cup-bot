use rusqlite::{Connection, OptionalExtension, params};

use crate::scoring::FinishedMatch;

pub struct WcMatchResult {
    pub pool_id: i64,
    pub match_id: i64,
    pub home_team_id: i64,
    pub away_team_id: i64,
    pub home_goals: i64,
    pub away_goals: i64,
    pub stage: Option<String>,
    pub finished_at: Option<String>,
}

impl WcMatchResult {
    pub fn upsert(&self, conn: &Connection) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT INTO wc_match_results (
                pool_id, match_id, home_team_id, away_team_id, home_goals, away_goals, stage
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(pool_id, match_id) DO UPDATE SET
                home_team_id = excluded.home_team_id,
                away_team_id = excluded.away_team_id,
                home_goals = excluded.home_goals,
                away_goals = excluded.away_goals,
                stage = excluded.stage
            ",
            params![
                self.pool_id,
                self.match_id,
                self.home_team_id,
                self.away_team_id,
                self.home_goals,
                self.away_goals,
                self.stage,
            ],
        )?;
        Ok(())
    }

    pub fn list_for_pool(conn: &Connection, pool_id: i64) -> rusqlite::Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "
            SELECT pool_id, match_id, home_team_id, away_team_id, home_goals, away_goals, stage, finished_at
            FROM wc_match_results
            WHERE pool_id = ?1
            ",
        )?;
        let rows = stmt.query_map(params![pool_id], row_from)?;
        rows.collect()
    }

    pub fn score(
        conn: &Connection,
        pool_id: i64,
        match_id: i64,
    ) -> rusqlite::Result<Option<(i64, i64)>> {
        conn.query_row(
            "
            SELECT home_goals, away_goals
            FROM wc_match_results
            WHERE pool_id = ?1 AND match_id = ?2
            ",
            params![pool_id, match_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
    }

    pub fn as_finished_match(&self) -> FinishedMatch {
        FinishedMatch {
            home_team_id: self.home_team_id,
            away_team_id: self.away_team_id,
            home_goals: self.home_goals,
            away_goals: self.away_goals,
        }
    }
}

fn row_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<WcMatchResult> {
    Ok(WcMatchResult {
        pool_id: row.get(0)?,
        match_id: row.get(1)?,
        home_team_id: row.get(2)?,
        away_team_id: row.get(3)?,
        home_goals: row.get(4)?,
        away_goals: row.get(5)?,
        stage: row.get(6)?,
        finished_at: row.get(7)?,
    })
}

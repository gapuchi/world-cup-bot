use rusqlite::{Connection, OptionalExtension, params};

use super::season::wc_season_id;

pub struct WcPlayerGoalTotal {
    pub season_id: i64,
    pub player_id: i64,
    pub goals: i64,
    pub updated_at: String,
}

impl WcPlayerGoalTotal {
    pub fn upsert_batch(
        conn: &Connection,
        totals: &[(i64, i64)],
        updated_at: &str,
    ) -> rusqlite::Result<()> {
        let season_id = wc_season_id(conn)?;
        for (player_id, goals) in totals {
            conn.execute(
                "
                INSERT INTO wc_player_goal_totals (season_id, player_id, goals, updated_at)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(season_id, player_id) DO UPDATE SET
                    goals = excluded.goals,
                    updated_at = excluded.updated_at
                ",
                params![season_id, player_id, goals, updated_at],
            )?;
        }
        Ok(())
    }

    pub fn goals_for_player(conn: &Connection, player_id: i64) -> rusqlite::Result<i64> {
        let season_id = wc_season_id(conn)?;
        conn.query_row(
            "
            SELECT goals
            FROM wc_player_goal_totals
            WHERE season_id = ?1 AND player_id = ?2
            ",
            params![season_id, player_id],
            |row| row.get(0),
        )
        .optional()
        .map(|goals| goals.unwrap_or(0))
    }
}

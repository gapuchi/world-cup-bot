use rusqlite::{Connection, OptionalExtension, params};

pub struct EplPlayerGoalTotal;

impl EplPlayerGoalTotal {
    pub fn upsert_batch(
        conn: &Connection,
        season_id: i64,
        totals: &[(i64, i64)],
        updated_at: &str,
    ) -> rusqlite::Result<()> {
        for (player_id, goals) in totals {
            conn.execute(
                "
                INSERT INTO epl_player_goal_totals (season_id, player_id, goals, updated_at)
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

    pub fn goals_for_player(
        conn: &Connection,
        season_id: i64,
        player_id: i64,
    ) -> rusqlite::Result<i64> {
        conn.query_row(
            "
            SELECT goals
            FROM epl_player_goal_totals
            WHERE season_id = ?1 AND player_id = ?2
            ",
            params![season_id, player_id],
            |row| row.get(0),
        )
        .optional()
        .map(|goals| goals.unwrap_or(0))
    }
}

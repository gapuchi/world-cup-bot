use rusqlite::{Connection, OptionalExtension, params};

pub struct WcTiebreakerPick {
    pub player_id: i64,
    pub player_name: String,
    pub team_name: String,
}

impl WcTiebreakerPick {
    pub fn upsert(
        conn: &Connection,
        pool_id: i64,
        user_id: u64,
        player_id: i64,
        player_name: &str,
        team_id: i64,
        team_name: &str,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT INTO wc_tiebreaker_picks (
                pool_id, user_id, player_id, player_name, team_id, team_name
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(pool_id, user_id) DO UPDATE SET
                player_id = excluded.player_id,
                player_name = excluded.player_name,
                team_id = excluded.team_id,
                team_name = excluded.team_name
            ",
            params![
                pool_id,
                user_id as i64,
                player_id,
                player_name,
                team_id,
                team_name,
            ],
        )?;
        Ok(())
    }

    pub fn get_for_user(
        conn: &Connection,
        pool_id: i64,
        user_id: u64,
    ) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "
            SELECT player_id, player_name, team_name
            FROM wc_tiebreaker_picks
            WHERE pool_id = ?1 AND user_id = ?2
            ",
            params![pool_id, user_id as i64],
            |row| {
                Ok(WcTiebreakerPick {
                    player_id: row.get(0)?,
                    player_name: row.get(1)?,
                    team_name: row.get(2)?,
                })
            },
        )
        .optional()
    }

    pub fn delete_for_team(
        conn: &Connection,
        pool_id: i64,
        user_id: u64,
        team_id: i64,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "
            DELETE FROM wc_tiebreaker_picks
            WHERE pool_id = ?1 AND user_id = ?2 AND team_id = ?3
            ",
            params![pool_id, user_id as i64, team_id],
        )?;
        Ok(())
    }
}

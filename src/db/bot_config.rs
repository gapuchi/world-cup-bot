use rusqlite::{Connection, OptionalExtension, params};

pub struct BotConfig {
    pub active_pool_id: i64,
}

impl BotConfig {
    pub fn get(conn: &Connection) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "SELECT active_pool_id FROM bot_config WHERE id = 1",
            [],
            |row| Ok(Self {
                active_pool_id: row.get(0)?,
            }),
        )
        .optional()
    }

    pub fn set_active_pool_id(conn: &Connection, pool_id: i64) -> rusqlite::Result<()> {
        conn.execute(
            "
            INSERT INTO bot_config (id, active_pool_id) VALUES (1, ?1)
            ON CONFLICT(id) DO UPDATE SET active_pool_id = excluded.active_pool_id
            ",
            params![pool_id],
        )?;
        Ok(())
    }
}

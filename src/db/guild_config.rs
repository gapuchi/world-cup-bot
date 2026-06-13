use rusqlite::{Connection, OptionalExtension, params};

use super::pool::Pool;

pub struct GuildConfig {
    pub guild_id: u64,
    pub default_pool_id: i64,
}

impl GuildConfig {
    pub fn get(conn: &Connection, guild_id: u64) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "SELECT guild_id, default_pool_id FROM guild_config WHERE guild_id = ?1",
            params![guild_id as i64],
            |row| {
                Ok(Self {
                    guild_id: row.get::<_, i64>(0)? as u64,
                    default_pool_id: row.get(1)?,
                })
            },
        )
        .optional()
    }

    pub fn set_default_pool_id(
        conn: &Connection,
        guild_id: u64,
        pool_id: i64,
    ) -> rusqlite::Result<()> {
        let pool = Pool::get(conn, pool_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        if pool.guild_id != guild_id {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }

        conn.execute(
            "
            INSERT INTO guild_config (guild_id, default_pool_id) VALUES (?1, ?2)
            ON CONFLICT(guild_id) DO UPDATE SET default_pool_id = excluded.default_pool_id
            ",
            params![guild_id as i64, pool_id],
        )?;
        Ok(())
    }
}

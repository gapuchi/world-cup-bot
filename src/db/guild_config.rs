use rusqlite::{Connection, OptionalExtension, params};

use super::season::Season;

pub struct GuildConfig {
    pub guild_id: u64,
    pub default_season_id: i64,
}

impl GuildConfig {
    pub fn get(conn: &Connection, guild_id: u64) -> rusqlite::Result<Option<Self>> {
        conn.query_row(
            "SELECT guild_id, default_season_id FROM guild_config WHERE guild_id = ?1",
            params![guild_id as i64],
            |row| {
                Ok(Self {
                    guild_id: row.get::<_, i64>(0)? as u64,
                    default_season_id: row.get(1)?,
                })
            },
        )
        .optional()
    }

    pub fn set_default_season_id(
        conn: &Connection,
        guild_id: u64,
        season_id: i64,
    ) -> rusqlite::Result<()> {
        let season = Season::get(conn, season_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        if season.guild_id != guild_id {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }

        conn.execute(
            "
            INSERT INTO guild_config (guild_id, default_season_id) VALUES (?1, ?2)
            ON CONFLICT(guild_id) DO UPDATE SET default_season_id = excluded.default_season_id
            ",
            params![guild_id as i64, season_id],
        )?;
        Ok(())
    }
}

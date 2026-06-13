use crate::{
    db::{Pool, SeasonDisplay},
    types::{Data, Error},
};

pub async fn season_message(data: &Data, guild_id: u64) -> Result<String, Error> {
    let conn = data.db.lock().await;
    let pool = Pool::default_for_guild(&conn, guild_id)?;
    let season = SeasonDisplay::for_pool(&conn, pool.id)?;
    Ok(format!(
        "This bot is tracking **{}** (`{}`) for **{}**.",
        season.name, season.slug, season.league_name
    ))
}

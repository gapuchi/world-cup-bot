use crate::{
    db::{Season, SeasonDisplay},
    types::{Data, Error},
};

pub async fn season_message(data: &Data, guild_id: u64) -> Result<String, Error> {
    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    let display = SeasonDisplay::for_season(&conn, season.id)?;
    Ok(format!(
        "This bot is tracking **{}** (`{}`, {}) for **{}**.",
        display.name, display.slug, display.season_year, display.league_name
    ))
}

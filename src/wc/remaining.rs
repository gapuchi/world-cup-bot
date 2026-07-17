use crate::{
    db::{league_competition_code, Season},
    soccar::{self, TeamClassification},
    types::{Data, Error},
};

pub enum FetchOutcome {
    NotWorldCup,
    Report(TeamClassification),
}

pub async fn list_for_guild(data: &Data, guild_id: u64) -> Result<FetchOutcome, Error> {
    let competition = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let league_slug = Season::league_slug_for(&conn, season.id)?;
        if league_slug != "wc" {
            return Ok(FetchOutcome::NotWorldCup);
        }
        league_competition_code(&league_slug)
    };

    let api = crate::wc::football_data(data);
    let teams = api.fetch_teams(&competition).await?;
    let matches = api.fetch_competition_matches(&competition).await?;

    Ok(FetchOutcome::Report(soccar::classify_teams(
        &teams, &matches,
    )))
}

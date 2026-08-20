use crate::{
    db::league_competition_code,
    league::CatalogTeam,
    types::{Data, Error},
};

use super::api::football_data;

pub async fn list_teams(data: &Data) -> Result<Vec<CatalogTeam>, Error> {
    let competition = league_competition_code("epl");
    let teams = football_data(data).fetch_teams(&competition).await?;
    Ok(teams
        .into_iter()
        .map(|team| CatalogTeam {
            id: team.id,
            name: team.name,
            short_name: team.short_name,
            code: team.tla,
        })
        .collect())
}

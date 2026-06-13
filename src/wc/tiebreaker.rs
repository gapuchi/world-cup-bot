use crate::{
    db::{Season, Registration, WcTiebreakerPick},
    soccar::{fetch_squads_for_teams, find_players},
    types::{Data, Error},
};

pub async fn pick_tiebreaker_player(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    player_query: &str,
) -> Result<String, Error> {
    let registrations = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        Registration::list_for_user(&conn, season.id, user_id)?
    };

    if registrations.is_empty() {
        return Ok(
            "Claim a team first with `/claim`, then pick a player from that squad.".into(),
        );
    }

    let teams: Vec<(i64, String)> = registrations
        .iter()
        .map(|registration| (registration.team_id, registration.team_name.clone()))
        .collect();

    let squad = fetch_squads_for_teams(&data.soccar_api(), &teams).await?;
    let matches = find_players(&squad, player_query);

    match matches.as_slice() {
        [] => Ok(format!(
            "Couldn't find a player matching \"{player_query}\" on your claimed teams. Try a more specific name."
        )),
        [selected] => {
            let conn = data.db.lock().await;
            let season = Season::default_for_guild(&conn, guild_id)?;
            WcTiebreakerPick::upsert(
                &conn,
                season.id,
                user_id,
                selected.player_id,
                &selected.player_name,
                selected.team_id,
                &selected.team_name,
            )?;
            Ok(format!(
                "Tie-breaker player set to **{}** ({})",
                selected.player_name, selected.team_name
            ))
        }
        _ => {
            let options: Vec<String> = matches
                .iter()
                .take(10)
                .map(|candidate| format!("**{}** ({})", candidate.player_name, candidate.team_name))
                .collect();
            Ok(format!(
                "Several players match \"{player_query}\". Be more specific:\n{}",
                options.join("\n")
            ))
        }
    }
}

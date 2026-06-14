use crate::api::{EspnApi, EspnApiError, NflGame, NflTeam};

/// Returns final scores when ESPN marks the game completed and both scores are present.
pub fn final_score(game: &NflGame) -> Option<(i64, i64)> {
    if !game.completed {
        return None;
    }
    match (game.home_score, game.away_score) {
        (Some(home), Some(away)) => Some((home, away)),
        _ => None,
    }
}

pub fn find_team<'a>(teams: &'a [NflTeam], query: &str) -> Option<&'a NflTeam> {
    let query = query.trim().to_lowercase();
    teams.iter().find(|team| {
        team.name.to_lowercase() == query
            || team
                .abbreviation
                .as_ref()
                .is_some_and(|code| code.to_lowercase() == query)
            || team.name.to_lowercase().contains(&query)
    })
}

#[derive(Debug, Clone)]
pub struct RosterPlayerMatch {
    pub player_id: i64,
    pub player_name: String,
    pub team_id: i64,
    pub team_name: String,
}

pub async fn fetch_rosters_for_teams(
    api: &EspnApi,
    teams: &[(i64, String)],
) -> Result<Vec<RosterPlayerMatch>, EspnApiError> {
    let mut players = Vec::new();
    for (team_id, team_name) in teams {
        let roster = api.fetch_team_roster(*team_id).await?;
        for player in roster {
            players.push(RosterPlayerMatch {
                player_id: player.id,
                player_name: player.name,
                team_id: *team_id,
                team_name: team_name.clone(),
            });
        }
    }
    Ok(players)
}

pub fn find_players<'a>(
    players: &'a [RosterPlayerMatch],
    query: &str,
) -> Vec<&'a RosterPlayerMatch> {
    let query = query.trim().to_lowercase();
    players
        .iter()
        .filter(|player| {
            let name = player.player_name.to_lowercase();
            name == query || name.contains(&query)
        })
        .collect()
}

/// Regular season runs Sep–Jan; include playoffs through mid-Feb.
pub fn season_date_range(season_year: i64) -> String {
    format!("{}0901-{}0215", season_year, season_year + 1)
}

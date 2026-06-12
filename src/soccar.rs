use crate::api::{ApiError, FootballDataApi, Match, Team};

/// Returns full-time goals when the API has populated both sides.
/// football-data.org often marks matches FINISHED before scores are available.
pub fn full_time_score(m: &Match) -> Option<(i64, i64)> {
    match (m.score.full_time.home, m.score.full_time.away) {
        (Some(home), Some(away)) => Some((home, away)),
        _ => None,
    }
}

pub fn find_team<'a>(teams: &'a [Team], query: &str) -> Option<&'a Team> {
    let query = query.trim().to_lowercase();
    teams.iter().find(|team| {
        team.name.to_lowercase() == query
            || team
                .short_name
                .as_ref()
                .is_some_and(|n| n.to_lowercase() == query)
            || team.tla.as_ref().is_some_and(|t| t.to_lowercase() == query)
            || team.name.to_lowercase().contains(&query)
    })
}

#[derive(Debug, Clone)]
pub struct SquadPlayerMatch {
    pub player_id: i64,
    pub player_name: String,
    pub team_id: i64,
    pub team_name: String,
}

pub async fn fetch_squads_for_teams(
    api: &FootballDataApi,
    teams: &[(i64, String)],
) -> Result<Vec<SquadPlayerMatch>, ApiError> {
    let mut players = Vec::new();
    for (team_id, team_name) in teams {
        let squad = api.fetch_team_squad(*team_id).await?;
        for player in squad {
            if player
                .role
                .as_ref()
                .is_some_and(|role| role != "PLAYER")
            {
                continue;
            }
            players.push(SquadPlayerMatch {
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
    players: &'a [SquadPlayerMatch],
    query: &str,
) -> Vec<&'a SquadPlayerMatch> {
    let query = query.trim().to_lowercase();
    players
        .iter()
        .filter(|player| {
            let name = player.player_name.to_lowercase();
            name == query || name.contains(&query)
        })
        .collect()
}

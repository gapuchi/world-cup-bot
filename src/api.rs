use serde::Deserialize;

const COMPETITION: &str = "WC";
const BASE_URL: &str = "https://api.football-data.org/v4";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub short_name: Option<String>,
    pub tla: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreDetail {
    pub home: Option<i64>,
    pub away: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub full_time: ScoreDetail,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub id: i64,
    pub home_team: Team,
    pub away_team: Team,
    pub score: Score,
    pub stage: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MatchesResponse {
    pub matches: Vec<Match>,
}

#[derive(Debug, Deserialize)]
struct TeamsResponse {
    pub teams: Vec<Team>,
}

pub async fn fetch_teams(
    client: &reqwest::Client,
    token: &str,
) -> Result<Vec<Team>, reqwest::Error> {
    let url = format!("{BASE_URL}/competitions/{COMPETITION}/teams");
    let response = client
        .get(url)
        .header("X-Auth-Token", token)
        .send()
        .await?
        .error_for_status()?;
    let body: TeamsResponse = response.json().await?;
    Ok(body.teams)
}

pub async fn fetch_finished_matches(
    client: &reqwest::Client,
    token: &str,
) -> Result<Vec<Match>, reqwest::Error> {
    let url = format!("{BASE_URL}/competitions/{COMPETITION}/matches?status=FINISHED");
    let response = client
        .get(url)
        .header("X-Auth-Token", token)
        .send()
        .await?
        .error_for_status()?;
    let body: MatchesResponse = response.json().await?;
    Ok(body.matches)
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadPlayer {
    pub id: i64,
    pub name: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SquadPlayerMatch {
    pub player_id: i64,
    pub player_name: String,
    pub team_id: i64,
    pub team_name: String,
}

#[derive(Debug, Deserialize)]
struct TeamDetailResponse {
    squad: Vec<SquadPlayer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScorerEntry {
    player: ScorerPlayer,
    goals: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScorerPlayer {
    id: i64,
    name: String,
}

#[derive(Debug, Deserialize)]
struct ScorersResponse {
    scorers: Vec<ScorerEntry>,
}

pub async fn fetch_team_squad(
    client: &reqwest::Client,
    token: &str,
    team_id: i64,
) -> Result<Vec<SquadPlayer>, reqwest::Error> {
    let url = format!("{BASE_URL}/teams/{team_id}");
    let response = client
        .get(url)
        .header("X-Auth-Token", token)
        .send()
        .await?
        .error_for_status()?;
    let body: TeamDetailResponse = response.json().await?;
    Ok(body.squad)
}

pub async fn fetch_squads_for_teams(
    client: &reqwest::Client,
    token: &str,
    teams: &[(i64, String)],
) -> Result<Vec<SquadPlayerMatch>, reqwest::Error> {
    let mut players = Vec::new();
    for (team_id, team_name) in teams {
        let squad = fetch_team_squad(client, token, *team_id).await?;
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

pub async fn fetch_scorers(
    client: &reqwest::Client,
    token: &str,
) -> Result<Vec<(i64, i64)>, reqwest::Error> {
    let url = format!("{BASE_URL}/competitions/{COMPETITION}/scorers");
    let response = client
        .get(url)
        .header("X-Auth-Token", token)
        .send()
        .await?
        .error_for_status()?;
    let body: ScorersResponse = response.json().await?;
    Ok(body
        .scorers
        .into_iter()
        .map(|entry| (entry.player.id, entry.goals))
        .collect())
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

use std::fmt;

use reqwest::StatusCode;
use serde::Deserialize;

const SITE_BASE_URL: &str = "https://site.api.espn.com/apis/site/v2/sports/football/nfl";
const CORE_BASE_URL: &str = "https://sports.core.api.espn.com/v2/sports/football/leagues/nfl";

#[derive(Debug)]
pub enum EspnApiError {
    RateLimited,
    InvalidData(String),
    Request(reqwest::Error),
}

impl fmt::Display for EspnApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EspnApiError::RateLimited => {
                write!(f, "The ESPN API is rate-limited right now. Please try again later.")
            }
            EspnApiError::InvalidData(message) => write!(f, "{message}"),
            EspnApiError::Request(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for EspnApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EspnApiError::RateLimited | EspnApiError::InvalidData(_) => None,
            EspnApiError::Request(error) => Some(error),
        }
    }
}

fn check_response(response: reqwest::Response) -> Result<reqwest::Response, EspnApiError> {
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(EspnApiError::RateLimited);
    }
    response.error_for_status().map_err(EspnApiError::Request)
}

#[derive(Debug, Clone)]
pub struct NflTeam {
    pub id: i64,
    pub name: String,
    pub abbreviation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NflGame {
    pub id: i64,
    pub home_team: NflTeam,
    pub away_team: NflTeam,
    pub home_score: Option<i64>,
    pub away_score: Option<i64>,
    pub completed: bool,
}

#[derive(Debug, Clone)]
pub struct NflRosterPlayer {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct NflTouchdownLeader {
    pub player_id: i64,
    pub touchdowns: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiRef {
    #[serde(rename = "$ref")]
    href: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamBody {
    id: String,
    display_name: String,
    abbreviation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CompetitorBody {
    home_away: String,
    score: Option<String>,
    team: TeamBody,
}

#[derive(Debug, Clone, Deserialize)]
struct StatusTypeBody {
    completed: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct StatusBody {
    #[serde(rename = "type")]
    status_type: StatusTypeBody,
}

#[derive(Debug, Clone, Deserialize)]
struct CompetitionBody {
    competitors: Vec<CompetitorBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct EventBody {
    id: String,
    status: StatusBody,
    competitions: Vec<CompetitionBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct ScoreboardBody {
    events: Vec<EventBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct TeamsWrapper {
    team: TeamBody,
}

#[derive(Debug, Clone, Deserialize)]
struct LeagueBody {
    teams: Vec<TeamsWrapper>,
}

#[derive(Debug, Clone, Deserialize)]
struct TeamsResponseBody {
    sports: Vec<SportBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct SportBody {
    leagues: Vec<LeagueBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct RosterGroupBody {
    items: Vec<RosterPlayerBody>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RosterPlayerBody {
    id: String,
    display_name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RosterResponseBody {
    athletes: Vec<RosterGroupBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct LeaderEntryBody {
    value: f64,
    athlete: ApiRef,
}

#[derive(Debug, Clone, Deserialize)]
struct LeaderCategoryBody {
    name: String,
    leaders: Vec<LeaderEntryBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct LeadersResponseBody {
    categories: Vec<LeaderCategoryBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct AthleteResponseBody {
    id: String,
}

#[derive(Clone)]
pub struct EspnApi {
    client: reqwest::Client,
}

impl EspnApi {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    async fn get(&self, url: &str) -> Result<reqwest::Response, EspnApiError> {
        let response = self.client.get(url).send().await.map_err(EspnApiError::Request)?;
        check_response(response)
    }

    pub async fn fetch_teams(&self) -> Result<Vec<NflTeam>, EspnApiError> {
        let url = format!("{SITE_BASE_URL}/teams");
        let response = self.get(&url).await?;
        let body: TeamsResponseBody = response.json().await.map_err(EspnApiError::Request)?;
        let teams = body
            .sports
            .into_iter()
            .flat_map(|sport| sport.leagues)
            .flat_map(|league| league.teams)
            .filter_map(|entry| team_from_body(entry.team))
            .collect();
        Ok(teams)
    }

    pub async fn fetch_scoreboard(
        &self,
        season_year: i64,
        seasontype: i64,
        dates: Option<&str>,
    ) -> Result<Vec<NflGame>, EspnApiError> {
        let mut url = format!(
            "{SITE_BASE_URL}/scoreboard?season={season_year}&seasontype={seasontype}&limit=400"
        );
        if let Some(dates) = dates {
            url.push_str(&format!("&dates={dates}"));
        }
        let response = self.get(&url).await?;
        let body: ScoreboardBody = response.json().await.map_err(EspnApiError::Request)?;
        Ok(body.events.into_iter().filter_map(game_from_event).collect())
    }

    pub async fn fetch_team_roster(&self, team_id: i64) -> Result<Vec<NflRosterPlayer>, EspnApiError> {
        let url = format!("{SITE_BASE_URL}/teams/{team_id}/roster");
        let response = self.get(&url).await?;
        let body: RosterResponseBody = response.json().await.map_err(EspnApiError::Request)?;
        Ok(body
            .athletes
            .into_iter()
            .flat_map(|group| group.items)
            .filter_map(|player| {
                let id = player.id.parse().ok()?;
                Some(NflRosterPlayer {
                    id,
                    name: player.display_name,
                })
            })
            .collect())
    }

    pub async fn fetch_touchdown_leaders(
        &self,
        season_year: i64,
        seasontype: i64,
    ) -> Result<Vec<NflTouchdownLeader>, EspnApiError> {
        let url = format!(
            "{CORE_BASE_URL}/seasons/{season_year}/types/{seasontype}/leaders?limit=100"
        );
        let response = self.get(&url).await?;
        let body: LeadersResponseBody = response.json().await.map_err(EspnApiError::Request)?;
        let Some(category) = body
            .categories
            .into_iter()
            .find(|category| category.name == "totalTouchdowns")
        else {
            return Ok(Vec::new());
        };

        let mut leaders = Vec::with_capacity(category.leaders.len());
        for entry in category.leaders {
            let player_id = self.resolve_athlete_id(&entry.athlete.href).await?;
            leaders.push(NflTouchdownLeader {
                player_id,
                touchdowns: entry.value.round() as i64,
            });
        }
        Ok(leaders)
    }

    async fn resolve_athlete_id(&self, href: &str) -> Result<i64, EspnApiError> {
        let response = self.get(href).await?;
        let body: AthleteResponseBody = response.json().await.map_err(EspnApiError::Request)?;
        body.id.parse().map_err(|_| {
            EspnApiError::InvalidData(format!("invalid athlete id: {}", body.id))
        })
    }
}

fn parse_id(raw: &str) -> Option<i64> {
    raw.parse().ok()
}

fn parse_score(raw: Option<&String>) -> Option<i64> {
    raw.as_ref()?.parse().ok()
}

fn team_from_body(body: TeamBody) -> Option<NflTeam> {
    Some(NflTeam {
        id: parse_id(&body.id)?,
        name: body.display_name,
        abbreviation: body.abbreviation,
    })
}

fn game_from_event(event: EventBody) -> Option<NflGame> {
    let competition = event.competitions.first()?;
    let mut home_team = None;
    let mut away_team = None;
    let mut home_score = None;
    let mut away_score = None;

    for competitor in &competition.competitors {
        let team = team_from_body(competitor.team.clone())?;
        let score = parse_score(competitor.score.as_ref());
        match competitor.home_away.as_str() {
            "home" => {
                home_team = Some(team);
                home_score = score;
            }
            "away" => {
                away_team = Some(team);
                away_score = score;
            }
            _ => {}
        }
    }

    Some(NflGame {
        id: parse_id(&event.id)?,
        home_team: home_team?,
        away_team: away_team?,
        home_score,
        away_score,
        completed: event.status.status_type.completed,
    })
}

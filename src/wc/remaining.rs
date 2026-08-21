use std::collections::HashMap;

use crate::{
    db::{league_competition_code, Registration, Season},
    soccar::{self, TeamClassification, TeamRef},
    types::{Data, Error},
};

pub enum RemainingResult {
    WrongLeague,
    NoRegistrations,
    Report(RemainingReport),
}

pub struct RemainingReport {
    pub still_in_by_user: Vec<(u64, Vec<String>)>,
    pub unassigned_still_in: Vec<String>,
}

pub async fn list_for_guild(data: &Data, guild_id: u64) -> Result<RemainingResult, Error> {
    let (competition, season_id) = {
        let conn = data.db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let league_slug = Season::league_slug_for(&conn, season.id)?;
        if league_slug != "wc" {
            return Ok(RemainingResult::WrongLeague);
        }
        (league_competition_code(&league_slug), season.id)
    };

    let api = crate::api::FootballDataApi::from_env(data.http.clone());
    let teams = api.fetch_teams(&competition).await?;
    let matches = api.fetch_competition_matches(&competition).await?;
    let classification = soccar::classify_teams(&teams, &matches);

    let registrations = {
        let conn = data.db.lock().await;
        Registration::list_for_season(&conn, season_id)?
    };

    if registrations.is_empty() {
        return Ok(RemainingResult::NoRegistrations);
    }

    Ok(RemainingResult::Report(build_report(
        &classification,
        &registrations,
    )))
}

fn build_report(
    classification: &TeamClassification,
    registrations: &[Registration],
) -> RemainingReport {
    let (still_in_by_user, unassigned_still_in) =
        group_teams_by_user(&classification.still_in, registrations);

    RemainingReport {
        still_in_by_user,
        unassigned_still_in,
    }
}

pub fn group_teams_by_user(
    teams: &[TeamRef],
    registrations: &[Registration],
) -> (Vec<(u64, Vec<String>)>, Vec<String>) {
    let owner_by_team: HashMap<i64, u64> = registrations
        .iter()
        .map(|registration| (registration.team_id, registration.user_id))
        .collect();

    let mut by_user: HashMap<u64, Vec<String>> = HashMap::new();
    let mut unassigned = Vec::new();

    for team in teams {
        if let Some(&user_id) = owner_by_team.get(&team.id) {
            by_user.entry(user_id).or_default().push(team.name.clone());
        } else {
            unassigned.push(team.name.clone());
        }
    }

    for names in by_user.values_mut() {
        names.sort();
    }
    unassigned.sort();

    let mut user_ids: Vec<u64> = by_user.keys().copied().collect();
    user_ids.sort_unstable();

    let grouped = user_ids
        .into_iter()
        .map(|user_id| (user_id, by_user.remove(&user_id).unwrap_or_default()))
        .collect();

    (grouped, unassigned)
}

pub fn format_grouped_field(by_user: &[(u64, Vec<String>)], unassigned: &[String]) -> String {
    let mut lines: Vec<String> = by_user
        .iter()
        .map(|(user_id, teams)| format!("<@{}> — **{}**", user_id, teams.join("**, **")))
        .collect();

    if !unassigned.is_empty() {
        lines.push(format!(
            "Unassigned — **{}**",
            unassigned.join("**, **")
        ));
    }

    if lines.is_empty() {
        "—".into()
    } else {
        lines.join("\n")
    }
}

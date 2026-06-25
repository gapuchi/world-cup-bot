use std::collections::HashMap;

use crate::{
    db::Registration,
    soccar::{TeamClassification, TeamRef},
    types::{Data, Error},
    wc::remaining::{self, FetchOutcome as WcFetchOutcome},
};

pub enum RemainingResult {
    NotWorldCup,
    NoRegistrations,
    Report(RemainingReport),
}

pub struct RemainingReport {
    pub still_in_by_user: Vec<(u64, Vec<String>)>,
    pub eliminated_by_user: Vec<(u64, Vec<String>)>,
    pub unassigned_still_in: Vec<String>,
    pub unassigned_eliminated: Vec<String>,
}

pub async fn list_for_guild(data: &Data, guild_id: u64) -> Result<RemainingResult, Error> {
    let classification = match remaining::list_for_guild(data, guild_id).await? {
        WcFetchOutcome::NotWorldCup => return Ok(RemainingResult::NotWorldCup),
        WcFetchOutcome::Report(classification) => classification,
    };

    let registrations = {
        let conn = data.db.lock().await;
        let season = crate::db::Season::default_for_guild(&conn, guild_id)?;
        Registration::list_for_season(&conn, season.id)?
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
        group_by_user(&classification.still_in, registrations);
    let (eliminated_by_user, unassigned_eliminated) =
        group_by_user(&classification.eliminated, registrations);

    RemainingReport {
        still_in_by_user,
        eliminated_by_user,
        unassigned_still_in,
        unassigned_eliminated,
    }
}

fn group_by_user(
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
    user_ids.sort();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soccar::TeamRef;

    fn registration(user_id: u64, team_id: i64, name: &str) -> Registration {
        Registration {
            user_id,
            team_id,
            team_name: name.into(),
        }
    }

    #[test]
    fn groups_teams_by_registered_owner() {
        let teams = vec![
            TeamRef {
                id: 1,
                name: "Brazil".into(),
            },
            TeamRef {
                id: 2,
                name: "Mexico".into(),
            },
            TeamRef {
                id: 3,
                name: "Japan".into(),
            },
        ];
        let registrations = vec![
            registration(10, 1, "Brazil"),
            registration(10, 2, "Mexico"),
            registration(20, 3, "Japan"),
        ];

        let (by_user, unassigned) = group_by_user(&teams, &registrations);
        assert!(unassigned.is_empty());
        assert_eq!(by_user.len(), 2);
        assert_eq!(by_user[0], (10, vec!["Brazil".into(), "Mexico".into()]));
        assert_eq!(by_user[1], (20, vec!["Japan".into()]));
    }

    #[test]
    fn unassigned_teams_listed_separately() {
        let teams = vec![TeamRef {
            id: 99,
            name: "Turkey".into(),
        }];
        let (by_user, unassigned) = group_by_user(&teams, &[]);
        assert!(by_user.is_empty());
        assert_eq!(unassigned, vec!["Turkey".to_string()]);
    }
}

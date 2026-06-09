use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{api::Match, scoring::FinishedMatch};

pub struct Registration {
    pub user_id: u64,
    pub team_id: i64,
    pub team_name: String,
}

pub struct BotConfig {
    pub announce_channel_id: u64,
}

pub struct TiebreakerPick {
    pub user_id: u64,
    pub player_id: i64,
    pub player_name: String,
    pub team_id: i64,
    pub team_name: String,
}

pub struct StandingRow {
    pub user_id: u64,
    pub points: i64,
    pub teams: Vec<(String, i64)>,
    pub tiebreaker_goals: i64,
    pub tiebreaker_player: Option<String>,
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            announce_channel_id INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS registrations (
            user_id INTEGER NOT NULL,
            team_id INTEGER PRIMARY KEY NOT NULL,
            team_name TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS match_results (
            match_id INTEGER PRIMARY KEY NOT NULL,
            home_team_id INTEGER NOT NULL,
            away_team_id INTEGER NOT NULL,
            home_goals INTEGER NOT NULL,
            away_goals INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS processed_matches (
            match_id INTEGER PRIMARY KEY NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tiebreaker_picks (
            user_id INTEGER PRIMARY KEY NOT NULL,
            player_id INTEGER NOT NULL,
            player_name TEXT NOT NULL,
            team_id INTEGER NOT NULL,
            team_name TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS player_goal_totals (
            player_id INTEGER PRIMARY KEY NOT NULL,
            goals INTEGER NOT NULL,
            updated_at TEXT NOT NULL
        );
        ",
    )
}

pub fn set_announce_channel(conn: &Connection, channel_id: u64) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO config (id, announce_channel_id)
        VALUES (1, ?1)
        ON CONFLICT(id) DO UPDATE SET announce_channel_id = excluded.announce_channel_id
        ",
        params![channel_id as i64],
    )?;
    Ok(())
}

pub fn get_config(conn: &Connection) -> rusqlite::Result<Option<BotConfig>> {
    conn.query_row(
        "SELECT announce_channel_id FROM config WHERE id = 1",
        [],
        |row| {
            Ok(BotConfig {
                announce_channel_id: row.get::<_, i64>(0)? as u64,
            })
        },
    )
    .optional()
}

pub fn register_team(
    conn: &Connection,
    user_id: u64,
    team_id: i64,
    team_name: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO registrations (user_id, team_id, team_name)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(team_id) DO UPDATE SET
            user_id = excluded.user_id,
            team_name = excluded.team_name
        ",
        params![user_id as i64, team_id, team_name],
    )?;
    Ok(())
}

pub fn unregister_team(conn: &Connection, user_id: u64, team_id: i64) -> rusqlite::Result<bool> {
    conn.execute(
        "DELETE FROM tiebreaker_picks WHERE user_id = ?1 AND team_id = ?2",
        params![user_id as i64, team_id],
    )?;
    let changed = conn.execute(
        "DELETE FROM registrations WHERE user_id = ?1 AND team_id = ?2",
        params![user_id as i64, team_id],
    )?;
    Ok(changed > 0)
}

pub fn list_user_registrations(
    conn: &Connection,
    user_id: u64,
) -> rusqlite::Result<Vec<Registration>> {
    let mut stmt = conn.prepare(
        "SELECT user_id, team_id, team_name FROM registrations WHERE user_id = ?1 ORDER BY team_name",
    )?;
    let rows = stmt.query_map(params![user_id as i64], |row| {
        Ok(Registration {
            user_id: row.get::<_, i64>(0)? as u64,
            team_id: row.get(1)?,
            team_name: row.get(2)?,
        })
    })?;
    rows.collect()
}

pub fn get_registration_by_team(
    conn: &Connection,
    team_id: i64,
) -> rusqlite::Result<Option<Registration>> {
    conn.query_row(
        "SELECT user_id, team_id, team_name FROM registrations WHERE team_id = ?1",
        params![team_id],
        |row| {
            Ok(Registration {
                user_id: row.get::<_, i64>(0)? as u64,
                team_id: row.get(1)?,
                team_name: row.get(2)?,
            })
        },
    )
    .optional()
}

pub fn list_registrations(conn: &Connection) -> rusqlite::Result<Vec<Registration>> {
    let mut stmt = conn.prepare(
        "SELECT user_id, team_id, team_name FROM registrations ORDER BY team_name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Registration {
            user_id: row.get::<_, i64>(0)? as u64,
            team_id: row.get(1)?,
            team_name: row.get(2)?,
        })
    })?;
    rows.collect()
}

pub fn upsert_match_result(conn: &Connection, m: &Match) -> rusqlite::Result<()> {
    let home_goals = m.score.full_time.home.unwrap_or(0);
    let away_goals = m.score.full_time.away.unwrap_or(0);

    conn.execute(
        "
        INSERT INTO match_results (match_id, home_team_id, away_team_id, home_goals, away_goals)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(match_id) DO UPDATE SET
            home_team_id = excluded.home_team_id,
            away_team_id = excluded.away_team_id,
            home_goals = excluded.home_goals,
            away_goals = excluded.away_goals
        ",
        params![
            m.id,
            m.home_team.id,
            m.away_team.id,
            home_goals,
            away_goals,
        ],
    )?;
    Ok(())
}

pub fn list_match_results(conn: &Connection) -> rusqlite::Result<Vec<FinishedMatch>> {
    let mut stmt = conn.prepare(
        "
        SELECT home_team_id, away_team_id, home_goals, away_goals
        FROM match_results
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(FinishedMatch {
            home_team_id: row.get(0)?,
            away_team_id: row.get(1)?,
            home_goals: row.get(2)?,
            away_goals: row.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn set_tiebreaker_pick(
    conn: &Connection,
    user_id: u64,
    player_id: i64,
    player_name: &str,
    team_id: i64,
    team_name: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO tiebreaker_picks (user_id, player_id, player_name, team_id, team_name)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(user_id) DO UPDATE SET
            player_id = excluded.player_id,
            player_name = excluded.player_name,
            team_id = excluded.team_id,
            team_name = excluded.team_name
        ",
        params![
            user_id as i64,
            player_id,
            player_name,
            team_id,
            team_name,
        ],
    )?;
    Ok(())
}

pub fn get_tiebreaker_pick(
    conn: &Connection,
    user_id: u64,
) -> rusqlite::Result<Option<TiebreakerPick>> {
    conn.query_row(
        "
        SELECT user_id, player_id, player_name, team_id, team_name
        FROM tiebreaker_picks
        WHERE user_id = ?1
        ",
        params![user_id as i64],
        |row| {
            Ok(TiebreakerPick {
                user_id: row.get::<_, i64>(0)? as u64,
                player_id: row.get(1)?,
                player_name: row.get(2)?,
                team_id: row.get(3)?,
                team_name: row.get(4)?,
            })
        },
    )
    .optional()
}

pub fn player_goal_total(conn: &Connection, player_id: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT goals FROM player_goal_totals WHERE player_id = ?1",
        params![player_id],
        |row| row.get(0),
    )
    .optional()
    .map(|goals| goals.unwrap_or(0))
}

pub fn tiebreaker_goals_for_user(conn: &Connection, user_id: u64) -> rusqlite::Result<i64> {
    let Some(pick) = get_tiebreaker_pick(conn, user_id)? else {
        return Ok(0);
    };
    player_goal_total(conn, pick.player_id)
}

pub fn upsert_player_goal_totals(
    conn: &Connection,
    totals: &[(i64, i64)],
    updated_at: &str,
) -> rusqlite::Result<()> {
    for (player_id, goals) in totals {
        conn.execute(
            "
            INSERT INTO player_goal_totals (player_id, goals, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(player_id) DO UPDATE SET
                goals = excluded.goals,
                updated_at = excluded.updated_at
            ",
            params![player_id, goals, updated_at],
        )?;
    }
    Ok(())
}

pub fn user_points(conn: &Connection, user_id: u64) -> rusqlite::Result<i64> {
    let matches = list_match_results(conn)?;
    let registrations = list_user_registrations(conn, user_id)?;
    let team_ids: Vec<i64> = registrations.iter().map(|r| r.team_id).collect();
    Ok(crate::scoring::points_for_teams(&team_ids, &matches))
}

pub fn get_standings(conn: &Connection) -> rusqlite::Result<Vec<StandingRow>> {
    let matches = list_match_results(conn)?;
    let registrations = list_registrations(conn)?;

    let mut by_user: HashMap<u64, (Vec<i64>, Vec<String>)> = HashMap::new();
    for registration in registrations {
        let entry = by_user.entry(registration.user_id).or_default();
        entry.0.push(registration.team_id);
        entry.1.push(registration.team_name);
    }

    let mut rows: Vec<StandingRow> = by_user
        .into_iter()
        .map(|(user_id, (team_ids, team_names))| {
            let pick = get_tiebreaker_pick(conn, user_id).ok().flatten();
            let tiebreaker_goals = pick
                .as_ref()
                .map(|pick| player_goal_total(conn, pick.player_id).unwrap_or(0))
                .unwrap_or(0);
            let mut teams: Vec<(String, i64)> = team_ids
                .iter()
                .zip(&team_names)
                .map(|(team_id, team_name)| {
                    (
                        team_name.clone(),
                        crate::scoring::points_for_team(*team_id, &matches),
                    )
                })
                .collect();
            teams.sort_by(|a, b| a.0.cmp(&b.0));
            StandingRow {
                user_id,
                points: crate::scoring::points_for_teams(&team_ids, &matches),
                teams,
                tiebreaker_goals,
                tiebreaker_player: pick.map(|pick| pick.player_name),
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then_with(|| b.tiebreaker_goals.cmp(&a.tiebreaker_goals))
            .then_with(|| a.user_id.cmp(&b.user_id))
    });
    Ok(rows)
}

pub fn is_match_processed(conn: &Connection, match_id: i64) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT 1 FROM processed_matches WHERE match_id = ?1",
        params![match_id],
        |_| Ok(()),
    )
    .optional()
    .map(|row| row.is_some())
}

pub fn mark_match_processed(conn: &Connection, match_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO processed_matches (match_id) VALUES (?1)",
        params![match_id],
    )?;
    Ok(())
}

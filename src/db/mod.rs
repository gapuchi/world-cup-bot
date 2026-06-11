mod migrate;
mod pool;

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

pub use pool::{ensure_wc_pool, get_wc_season, list_wc_pools, set_announce_channel};

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
    migrate::run(conn)
}

pub fn get_config(conn: &Connection, pool_id: i64) -> rusqlite::Result<Option<BotConfig>> {
    let channel: Option<i64> = conn
        .query_row(
            "SELECT announce_channel_id FROM pools WHERE id = ?1",
            params![pool_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    Ok(channel.map(|id| BotConfig {
        announce_channel_id: id as u64,
    }))
}

pub fn register_team(
    conn: &Connection,
    pool_id: i64,
    user_id: u64,
    team_id: i64,
    team_name: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO teams (league_id, team_id, name)
        VALUES (1, ?1, ?2)
        ON CONFLICT(league_id, team_id) DO UPDATE SET name = excluded.name
        ",
        params![team_id, team_name],
    )?;
    conn.execute(
        "
        INSERT INTO registrations (pool_id, user_id, team_id, team_name)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(pool_id, team_id) DO UPDATE SET
            user_id = excluded.user_id,
            team_name = excluded.team_name
        ",
        params![pool_id, user_id as i64, team_id, team_name],
    )?;
    Ok(())
}

pub fn unregister_team(
    conn: &Connection,
    pool_id: i64,
    user_id: u64,
    team_id: i64,
) -> rusqlite::Result<bool> {
    conn.execute(
        "
        DELETE FROM wc_tiebreaker_picks
        WHERE pool_id = ?1 AND user_id = ?2 AND team_id = ?3
        ",
        params![pool_id, user_id as i64, team_id],
    )?;
    let changed = conn.execute(
        "DELETE FROM registrations WHERE pool_id = ?1 AND user_id = ?2 AND team_id = ?3",
        params![pool_id, user_id as i64, team_id],
    )?;
    Ok(changed > 0)
}

pub fn list_user_registrations(
    conn: &Connection,
    pool_id: i64,
    user_id: u64,
) -> rusqlite::Result<Vec<Registration>> {
    let mut stmt = conn.prepare(
        "
        SELECT user_id, team_id, team_name
        FROM registrations
        WHERE pool_id = ?1 AND user_id = ?2
        ORDER BY team_name
        ",
    )?;
    let rows = stmt.query_map(params![pool_id, user_id as i64], |row| {
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
    pool_id: i64,
    team_id: i64,
) -> rusqlite::Result<Option<Registration>> {
    conn.query_row(
        "
        SELECT user_id, team_id, team_name
        FROM registrations
        WHERE pool_id = ?1 AND team_id = ?2
        ",
        params![pool_id, team_id],
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

pub fn list_registrations(
    conn: &Connection,
    pool_id: i64,
) -> rusqlite::Result<Vec<Registration>> {
    let mut stmt = conn.prepare(
        "
        SELECT user_id, team_id, team_name
        FROM registrations
        WHERE pool_id = ?1
        ORDER BY team_name
        ",
    )?;
    let rows = stmt.query_map(params![pool_id], |row| {
        Ok(Registration {
            user_id: row.get::<_, i64>(0)? as u64,
            team_id: row.get(1)?,
            team_name: row.get(2)?,
        })
    })?;
    rows.collect()
}

pub fn upsert_match_result(
    conn: &Connection,
    pool_id: i64,
    m: &Match,
) -> rusqlite::Result<()> {
    let Some((home_goals, away_goals)) = m.full_time_score() else {
        return Ok(());
    };

    conn.execute(
        "
        INSERT INTO wc_match_results (
            pool_id, match_id, home_team_id, away_team_id, home_goals, away_goals, stage
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(pool_id, match_id) DO UPDATE SET
            home_team_id = excluded.home_team_id,
            away_team_id = excluded.away_team_id,
            home_goals = excluded.home_goals,
            away_goals = excluded.away_goals,
            stage = excluded.stage
        ",
        params![
            pool_id,
            m.id,
            m.home_team.id,
            m.away_team.id,
            home_goals,
            away_goals,
            m.stage,
        ],
    )?;
    Ok(())
}

fn list_match_results(conn: &Connection, pool_id: i64) -> rusqlite::Result<Vec<FinishedMatch>> {
    let mut stmt = conn.prepare(
        "
        SELECT home_team_id, away_team_id, home_goals, away_goals
        FROM wc_match_results
        WHERE pool_id = ?1
        ",
    )?;
    let rows = stmt.query_map(params![pool_id], |row| {
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
    pool_id: i64,
    user_id: u64,
    player_id: i64,
    player_name: &str,
    team_id: i64,
    team_name: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO wc_tiebreaker_picks (
            pool_id, user_id, player_id, player_name, team_id, team_name
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(pool_id, user_id) DO UPDATE SET
            player_id = excluded.player_id,
            player_name = excluded.player_name,
            team_id = excluded.team_id,
            team_name = excluded.team_name
        ",
        params![
            pool_id,
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
    pool_id: i64,
    user_id: u64,
) -> rusqlite::Result<Option<TiebreakerPick>> {
    conn.query_row(
        "
        SELECT user_id, player_id, player_name, team_id, team_name
        FROM wc_tiebreaker_picks
        WHERE pool_id = ?1 AND user_id = ?2
        ",
        params![pool_id, user_id as i64],
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

fn wc_season_id(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "
        SELECT s.id
        FROM seasons s
        JOIN leagues l ON l.id = s.league_id
        WHERE l.slug = ?1 AND s.slug = ?2
        ",
        params![migrate::WC_LEAGUE_SLUG, migrate::WC_SEASON_SLUG],
        |row| row.get(0),
    )
}

fn player_goal_total(conn: &Connection, player_id: i64) -> rusqlite::Result<i64> {
    let season_id = wc_season_id(conn)?;
    conn.query_row(
        "
        SELECT goals
        FROM wc_player_goal_totals
        WHERE season_id = ?1 AND player_id = ?2
        ",
        params![season_id, player_id],
        |row| row.get(0),
    )
    .optional()
    .map(|goals| goals.unwrap_or(0))
}

pub fn tiebreaker_goals_for_user(
    conn: &Connection,
    pool_id: i64,
    user_id: u64,
) -> rusqlite::Result<i64> {
    let Some(pick) = get_tiebreaker_pick(conn, pool_id, user_id)? else {
        return Ok(0);
    };
    player_goal_total(conn, pick.player_id)
}

pub fn upsert_player_goal_totals(
    conn: &Connection,
    totals: &[(i64, i64)],
    updated_at: &str,
) -> rusqlite::Result<()> {
    let season_id = wc_season_id(conn)?;
    for (player_id, goals) in totals {
        conn.execute(
            "
            INSERT INTO wc_player_goal_totals (season_id, player_id, goals, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(season_id, player_id) DO UPDATE SET
                goals = excluded.goals,
                updated_at = excluded.updated_at
            ",
            params![season_id, player_id, goals, updated_at],
        )?;
    }
    Ok(())
}

pub fn user_points(
    conn: &Connection,
    pool_id: i64,
    user_id: u64,
) -> rusqlite::Result<i64> {
    let matches = list_match_results(conn, pool_id)?;
    let registrations = list_user_registrations(conn, pool_id, user_id)?;
    let team_ids: Vec<i64> = registrations.iter().map(|r| r.team_id).collect();
    Ok(crate::scoring::points_for_teams(&team_ids, &matches))
}

pub fn get_standings(conn: &Connection, pool_id: i64) -> rusqlite::Result<Vec<StandingRow>> {
    let matches = list_match_results(conn, pool_id)?;
    let registrations = list_registrations(conn, pool_id)?;

    let mut by_user: HashMap<u64, (Vec<i64>, Vec<String>)> = HashMap::new();
    for registration in registrations {
        let entry = by_user.entry(registration.user_id).or_default();
        entry.0.push(registration.team_id);
        entry.1.push(registration.team_name);
    }

    let mut rows: Vec<StandingRow> = by_user
        .into_iter()
        .map(|(user_id, (team_ids, team_names))| {
            let pick = get_tiebreaker_pick(conn, pool_id, user_id).ok().flatten();
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

pub fn get_match_score(
    conn: &Connection,
    pool_id: i64,
    match_id: i64,
) -> rusqlite::Result<Option<(i64, i64)>> {
    conn.query_row(
        "
        SELECT home_goals, away_goals
        FROM wc_match_results
        WHERE pool_id = ?1 AND match_id = ?2
        ",
        params![pool_id, match_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
}

pub fn unmark_match_processed(
    conn: &Connection,
    pool_id: i64,
    match_id: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM wc_processed_matches WHERE pool_id = ?1 AND match_id = ?2",
        params![pool_id, match_id],
    )?;
    Ok(())
}

pub fn is_match_processed(
    conn: &Connection,
    pool_id: i64,
    match_id: i64,
) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT 1 FROM wc_processed_matches WHERE pool_id = ?1 AND match_id = ?2",
        params![pool_id, match_id],
        |_| Ok(()),
    )
    .optional()
    .map(|row| row.is_some())
}

pub fn mark_match_processed(
    conn: &Connection,
    pool_id: i64,
    match_id: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT OR IGNORE INTO wc_processed_matches (pool_id, match_id)
        VALUES (?1, ?2)
        ",
        params![pool_id, match_id],
    )?;
    Ok(())
}

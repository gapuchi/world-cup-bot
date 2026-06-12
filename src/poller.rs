use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    api::Match,
    db::{Pool, PoolMeta, Registration, WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch},
    soccar::full_time_score,
    scoring::{self, DRAW_POINTS, LOSS_POINTS, WIN_POINTS},
    standings,
    types::Data,
};

struct MatchUpdate {
    user_id: u64,
    team_name: String,
    points_earned: i64,
    total_points: i64,
}

pub fn start_poller(data: Arc<Data>, cache_http: Arc<serenity::Http>) {
    tokio::spawn(async move {
        const POLL_INTERVAL: Duration = Duration::from_secs(300);
        // Let the Discord gateway finish connecting before the first HTTP request.
        tokio::time::sleep(Duration::from_secs(5)).await;

        loop {
            if let Err(error) = poll_once(&data, &cache_http).await {
                eprintln!("Poll failed: {error:#}");
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

async fn poll_once(
    data: &Data,
    http: &serenity::Http,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pool_metas = {
        let conn = data.db.lock().await;
        Pool::list_all_with_meta(&conn)?
    };

    if pool_metas.is_empty() {
        eprintln!("Poll complete: no pools configured");
        return Ok(());
    }

    let mut by_competition: HashMap<String, Vec<PoolMeta>> = HashMap::new();
    for meta in pool_metas {
        by_competition
            .entry(meta.external_season_id.clone())
            .or_default()
            .push(meta);
    }

    let mut total_matches = 0;
    let mut total_scored = 0;
    let mut total_pools = 0;

    for (competition, pools) in by_competition {
        let league_slug = pools[0].league_slug.clone();
        match league_slug.as_str() {
            "wc" => {
                let (matches, scored, scorers_line) =
                    poll_wc(data, http, &pools, &competition).await?;
                total_matches += matches;
                total_scored += scored;
                total_pools += pools.len();
                eprintln!(
                    "WC poll: {} finished match(es) ({} with scores), {} pool(s){scorers_line}",
                    matches,
                    scored,
                    pools.len(),
                    scorers_line = scorers_line,
                );
            }
            "nfl" => {
                eprintln!(
                    "NFL polling not implemented yet ({} pool(s) skipped)",
                    pools.len()
                );
            }
            other => {
                eprintln!(
                    "No poller for league \"{other}\" ({competition}, {} pool(s) skipped)",
                    pools.len()
                );
            }
        }
    }

    eprintln!(
        "Poll complete: {} finished match(es) ({} with scores), {} pool(s)",
        total_matches,
        total_scored,
        total_pools,
    );

    Ok(())
}

async fn poll_wc(
    data: &Data,
    http: &serenity::Http,
    pools: &[PoolMeta],
    competition: &str,
) -> Result<(usize, usize, String), Box<dyn std::error::Error + Send + Sync>> {
    let matches = data.soccar_api().fetch_finished_matches(competition).await?;

    {
        let conn = data.db.lock().await;
        for meta in pools {
            for m in &matches {
                if let Some(result) = wc_match_result_from_api(meta.pool.id, m) {
                    result.upsert(&conn)?;
                }
            }
        }
    }

    let season_id = pools[0].season_id;
    let scorers_line = match data.soccar_api().fetch_scorers(competition).await {
        Ok(scorers) => {
            let count = scorers.len();
            let conn = data.db.lock().await;
            let updated_at = chrono_lite_timestamp();
            let scorer_pairs: Vec<(i64, i64)> = scorers
                .into_iter()
                .map(|scorer| (scorer.player_id, scorer.goals))
                .collect();
            if let Err(error) =
                WcPlayerGoalTotal::upsert_batch(&conn, season_id, &scorer_pairs, &updated_at)
            {
                eprintln!("Failed to cache player goal totals: {error}");
                String::new()
            } else {
                format!(", {count} scorers cached")
            }
        }
        Err(error) => {
            eprintln!("Failed to fetch scorers for {competition}: {error}");
            String::new()
        }
    };

    for meta in pools {
        for m in &matches {
            if let Err(error) = process_wc_match(data, http, meta, m).await {
                eprintln!(
                    "Failed to process match {} for pool {}: {error}",
                    m.id,
                    meta.pool.id
                );
            }
        }
    }

    let scored_matches = matches
        .iter()
        .filter(|m| full_time_score(m).is_some())
        .count();

    Ok((matches.len(), scored_matches, scorers_line))
}

fn wc_match_result_from_api(pool_id: i64, m: &Match) -> Option<WcMatchResult> {
    let (home_goals, away_goals) = full_time_score(m)?;
    Some(WcMatchResult {
        pool_id,
        match_id: m.id,
        home_team_id: m.home_team.id,
        away_team_id: m.away_team.id,
        home_goals,
        away_goals,
        stage: m.stage.clone(),
    })
}

async fn process_wc_match(
    data: &Data,
    http: &serenity::Http,
    meta: &PoolMeta,
    m: &Match,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pool = &meta.pool;
    let Some((home_goals, away_goals)) = full_time_score(m) else {
        return Ok(());
    };

    let updates = {
        let conn = data.db.lock().await;
        if WcProcessedMatch::is_processed(&conn, pool.id, m.id)? {
            let stored = WcMatchResult::score(&conn, pool.id, m.id)?;
            if stored == Some((home_goals, away_goals)) {
                return Ok(());
            }
            WcProcessedMatch::unmark(&conn, pool.id, m.id)?;
        }

        let finished = scoring::FinishedMatch {
            home_team_id: m.home_team.id,
            away_team_id: m.away_team.id,
            home_goals,
            away_goals,
        };

        let mut updates = Vec::new();

        if let Some(registration) =
            Registration::get_by_team(&conn, pool.id, m.home_team.id)?
        {
            let points = scoring::points_for_team_in_match(registration.team_id, &finished);
            let total = standings::user_points(&conn, pool.id, registration.user_id)?;
            updates.push(MatchUpdate {
                user_id: registration.user_id,
                team_name: registration.team_name,
                points_earned: points,
                total_points: total,
            });
        }

        if let Some(registration) =
            Registration::get_by_team(&conn, pool.id, m.away_team.id)?
        {
            let points = scoring::points_for_team_in_match(registration.team_id, &finished);
            let total = standings::user_points(&conn, pool.id, registration.user_id)?;
            updates.push(MatchUpdate {
                user_id: registration.user_id,
                team_name: registration.team_name,
                points_earned: points,
                total_points: total,
            });
        }

        if updates.is_empty() {
            WcProcessedMatch::mark(&conn, pool.id, m.id)?;
            return Ok(());
        }

        WcProcessedMatch::mark(&conn, pool.id, m.id)?;
        updates
    };

    let Some(channel_id) = pool.announce_channel_id else {
        return Ok(());
    };

    let score_line = format!("{home_goals}–{away_goals}");
    let stage = m
        .stage
        .as_deref()
        .unwrap_or(&meta.league_name);

    let mut description = format!(
        "**{}** {} **{}**\n\n",
        m.home_team.name, score_line, m.away_team.name
    );

    for update in &updates {
        let mention = serenity::UserId::new(update.user_id).mention();
        description.push_str(&format!(
            "{mention} ({}) +{} pts → **{}** total\n",
            update.team_name, update.points_earned, update.total_points
        ));
    }

    description.push_str(&format!(
        "\nScoring: win {WIN_POINTS}, draw {DRAW_POINTS}, loss {LOSS_POINTS}"
    ));

    let embed = serenity::CreateEmbed::default()
        .title(format!("{stage} — full time"))
        .description(description)
        .colour(serenity::Colour::DARK_GREEN);

    let channel = serenity::ChannelId::new(channel_id);
    channel
        .send_message(http, serenity::CreateMessage::new().embed(embed))
        .await?;

    Ok(())
}

fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

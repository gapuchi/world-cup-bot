use poise::serenity_prelude as serenity;
use serenity::Mentionable;
use std::sync::Arc;
use std::time::Duration;

use crate::{
    api::{self, Match},
    db::{Pool, Registration, WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch},
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
                eprintln!("World Cup poll failed: {error:#}");
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

async fn poll_once(
    data: &Data,
    http: &serenity::Http,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let matches = api::fetch_finished_matches(&data.http, &data.api_token).await?;

    let pools = {
        let conn = data.db.lock().await;
        Pool::list_wc(&conn)?
    };

    {
        let conn = data.db.lock().await;
        for pool in &pools {
            for m in &matches {
                if let Some(result) = wc_match_result_from_api(pool.id, m) {
                    result.upsert(&conn)?;
                }
            }
        }
    }

    let scorers_updated = match api::fetch_scorers(&data.http, &data.api_token).await {
        Ok(scorers) => {
            let count = scorers.len();
            let conn = data.db.lock().await;
            let updated_at = chrono_lite_timestamp();
            if let Err(error) = WcPlayerGoalTotal::upsert_batch(&conn, &scorers, &updated_at) {
                eprintln!("Failed to cache player goal totals: {error}");
                None
            } else {
                Some(count)
            }
        }
        Err(error) => {
            eprintln!("Failed to fetch World Cup scorers: {error}");
            None
        }
    };

    for pool in &pools {
        for m in &matches {
            if let Err(error) = process_match(data, http, pool, m).await {
                eprintln!(
                    "Failed to process match {} for pool {}: {error}",
                    m.id, pool.id
                );
            }
        }
    }

    let scored_matches = matches
        .iter()
        .filter(|m| m.full_time_score().is_some())
        .count();
    let scorers_line = match scorers_updated {
        Some(count) => format!(", {count} scorers cached"),
        None => String::new(),
    };
    eprintln!(
        "World Cup poll complete: {} finished match(es) ({} with scores), {} pool(s){scorers_line}",
        matches.len(),
        scored_matches,
        pools.len(),
    );

    Ok(())
}

fn wc_match_result_from_api(pool_id: i64, m: &Match) -> Option<WcMatchResult> {
    let (home_goals, away_goals) = m.full_time_score()?;
    Some(WcMatchResult {
        pool_id,
        match_id: m.id,
        home_team_id: m.home_team.id,
        away_team_id: m.away_team.id,
        home_goals,
        away_goals,
        stage: m.stage.clone(),
        finished_at: None,
    })
}

async fn process_match(
    data: &Data,
    http: &serenity::Http,
    pool: &Pool,
    m: &Match,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some((home_goals, away_goals)) = m.full_time_score() else {
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
    let stage = m.stage.as_deref().unwrap_or("World Cup");

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

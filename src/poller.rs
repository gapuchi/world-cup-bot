use poise::serenity_prelude as serenity;
use serenity::Mentionable;
use std::sync::Arc;
use std::time::Duration;

use crate::{
    api::{self, Match},
    db,
    scoring::{self, DRAW_POINTS, LOSS_POINTS, WIN_POINTS},
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

    {
        let conn = data.db.lock().await;
        for m in &matches {
            db::upsert_match_result(&conn, m)?;
        }
    }

    match api::fetch_scorers(&data.http, &data.api_token).await {
        Ok(scorers) => {
            let conn = data.db.lock().await;
            let updated_at = chrono_lite_timestamp();
            if let Err(error) = db::upsert_player_goal_totals(&conn, &scorers, &updated_at) {
                eprintln!("Failed to cache player goal totals: {error}");
            }
        }
        Err(error) => eprintln!("Failed to fetch World Cup scorers: {error}"),
    }

    for m in &matches {
        if let Err(error) = process_match(data, http, m).await {
            eprintln!("Failed to process match {}: {error}", m.id);
        }
    }

    Ok(())
}

async fn process_match(
    data: &Data,
    http: &serenity::Http,
    m: &Match,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home_goals = m.score.full_time.home.unwrap_or(0);
    let away_goals = m.score.full_time.away.unwrap_or(0);

    let updates = {
        let conn = data.db.lock().await;
        if db::is_match_processed(&conn, m.id)? {
            return Ok(());
        }

        let finished = scoring::FinishedMatch {
            home_team_id: m.home_team.id,
            away_team_id: m.away_team.id,
            home_goals,
            away_goals,
        };

        let mut updates = Vec::new();

        if let Some(registration) = db::get_registration_by_team(&conn, m.home_team.id)? {
            let points = scoring::points_for_team_in_match(registration.team_id, &finished);
            let total = db::user_points(&conn, registration.user_id)?;
            updates.push(MatchUpdate {
                user_id: registration.user_id,
                team_name: registration.team_name,
                points_earned: points,
                total_points: total,
            });
        }

        if let Some(registration) = db::get_registration_by_team(&conn, m.away_team.id)? {
            let points = scoring::points_for_team_in_match(registration.team_id, &finished);
            let total = db::user_points(&conn, registration.user_id)?;
            updates.push(MatchUpdate {
                user_id: registration.user_id,
                team_name: registration.team_name,
                points_earned: points,
                total_points: total,
            });
        }

        if updates.is_empty() {
            db::mark_match_processed(&conn, m.id)?;
            return Ok(());
        }

        db::mark_match_processed(&conn, m.id)?;
        updates
    };

    if let Some(config) = {
        let conn = data.db.lock().await;
        db::get_config(&conn)?
    } {
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

        let channel = serenity::ChannelId::new(config.announce_channel_id);
        channel
            .send_message(http, serenity::CreateMessage::new().embed(embed))
            .await?;
    }

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

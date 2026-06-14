use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    api::{Match, NflGame},
    db::{
        NflMatchResult, NflPlayerTouchdownTotal, NflProcessedGame, Registration, Season,
        SeasonMeta, WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch, league_competition_code,
    },
    gridiron::{final_score, season_date_range},
    scoring::{self, format_points, format_rules_footer, rules_for_league},
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
    let season_metas = {
        let conn = data.db.lock().await;
        Season::list_all_with_meta(&conn)?
    };

    if season_metas.is_empty() {
        eprintln!("Poll complete: no seasons configured");
        return Ok(());
    }

    let mut by_league: HashMap<String, Vec<SeasonMeta>> = HashMap::new();
    for meta in season_metas {
        by_league
            .entry(meta.league_slug.clone())
            .or_default()
            .push(meta);
    }

    let mut total_matches = 0;
    let mut total_scored = 0;
    let mut total_seasons = 0;

    for (league_slug, seasons) in by_league {
        let competition = league_competition_code(&league_slug);
        match league_slug.as_str() {
            "wc" => {
                let (matches, scored, scorers_line) =
                    poll_wc(data, http, &seasons, &competition).await?;
                total_matches += matches;
                total_scored += scored;
                total_seasons += seasons.len();
                eprintln!(
                    "WC poll: {} finished match(es) ({} with scores), {} season(s){scorers_line}",
                    matches,
                    scored,
                    seasons.len(),
                    scorers_line = scorers_line,
                );
            }
            "nfl" => {
                let (games, scored, leaders_line) = poll_nfl(data, http, &seasons).await?;
                total_matches += games;
                total_scored += scored;
                total_seasons += seasons.len();
                eprintln!(
                    "NFL poll: {} finished game(s) ({} with scores), {} season(s){leaders_line}",
                    games,
                    scored,
                    seasons.len(),
                    leaders_line = leaders_line,
                );
            }
            other => {
                eprintln!(
                    "No poller for league \"{other}\" ({competition}, {} season(s) skipped)",
                    seasons.len()
                );
            }
        }
    }

    eprintln!(
        "Poll complete: {} finished match(es) ({} with scores), {} season(s)",
        total_matches,
        total_scored,
        total_seasons,
    );

    Ok(())
}

async fn poll_wc(
    data: &Data,
    http: &serenity::Http,
    seasons: &[SeasonMeta],
    competition: &str,
) -> Result<(usize, usize, String), Box<dyn std::error::Error + Send + Sync>> {
    let matches = data.soccar_api().fetch_finished_matches(competition).await?;
    let rules = rules_for_league("wc");

    {
        let conn = data.db.lock().await;
        for meta in seasons {
            for m in &matches {
                if let Some(result) = wc_match_result_from_api(meta.season.id, m) {
                    result.upsert(&conn)?;
                }
            }
        }
    }

    let scorers_line = match data.soccar_api().fetch_scorers(competition).await {
        Ok(scorers) => {
            let count = scorers.len();
            let conn = data.db.lock().await;
            let updated_at = chrono_lite_timestamp();
            let scorer_pairs: Vec<(i64, i64)> = scorers
                .into_iter()
                .map(|scorer| (scorer.player_id, scorer.goals))
                .collect();
            let mut cached = 0;
            for meta in seasons {
                if WcPlayerGoalTotal::upsert_batch(
                    &conn,
                    meta.season.id,
                    &scorer_pairs,
                    &updated_at,
                )
                .is_ok()
                {
                    cached += 1;
                } else {
                    eprintln!(
                        "Failed to cache player goal totals for season {}",
                        meta.season.id
                    );
                }
            }
            if cached == 0 {
                String::new()
            } else {
                format!(", {count} scorers cached for {cached} season(s)")
            }
        }
        Err(error) => {
            eprintln!("Failed to fetch scorers for {competition}: {error}");
            String::new()
        }
    };

    for meta in seasons {
        for m in &matches {
            if let Err(error) =
                process_wc_match(data, http, meta, m, &rules, "wc").await
            {
                eprintln!(
                    "Failed to process match {} for season {}: {error}",
                    m.id,
                    meta.season.id
                );
            }
        }
    }

    let scored_matches = matches
        .iter()
        .filter(|m| crate::soccar::full_time_score(m).is_some())
        .count();

    Ok((matches.len(), scored_matches, scorers_line))
}

async fn poll_nfl(
    data: &Data,
    http: &serenity::Http,
    seasons: &[SeasonMeta],
) -> Result<(usize, usize, String), Box<dyn std::error::Error + Send + Sync>> {
    let mut games_by_id: HashMap<i64, NflGame> = HashMap::new();
    let season_year = seasons
        .first()
        .map(|meta| meta.season.season_year)
        .unwrap_or(2025);
    let dates = season_date_range(season_year);

    for seasontype in [2_i64, 3] {
        let batch = data
            .espn_api()
            .fetch_scoreboard(season_year, seasontype, Some(&dates))
            .await?;
        for game in batch {
            games_by_id.insert(game.id, game);
        }
    }

    let games: Vec<NflGame> = games_by_id.into_values().collect();
    let rules = rules_for_league("nfl");

    {
        let conn = data.db.lock().await;
        for meta in seasons {
            for game in &games {
                if let Some(result) = nfl_match_result_from_api(meta.season.id, game) {
                    result.upsert(&conn)?;
                }
            }
        }
    }

    let leaders_line = match data
        .espn_api()
        .fetch_touchdown_leaders(season_year, 2)
        .await
    {
        Ok(leaders) => {
            let count = leaders.len();
            let conn = data.db.lock().await;
            let updated_at = chrono_lite_timestamp();
            let pairs: Vec<(i64, i64)> = leaders
                .into_iter()
                .map(|leader| (leader.player_id, leader.touchdowns))
                .collect();
            let mut cached = 0;
            for meta in seasons {
                if NflPlayerTouchdownTotal::upsert_batch(
                    &conn,
                    meta.season.id,
                    &pairs,
                    &updated_at,
                )
                .is_ok()
                {
                    cached += 1;
                } else {
                    eprintln!(
                        "Failed to cache touchdown totals for season {}",
                        meta.season.id
                    );
                }
            }
            if cached == 0 {
                String::new()
            } else {
                format!(", {count} TD leaders cached for {cached} season(s)")
            }
        }
        Err(error) => {
            eprintln!("Failed to fetch NFL touchdown leaders for {season_year}: {error}");
            String::new()
        }
    };

    for meta in seasons {
        for game in &games {
            if let Err(error) =
                process_nfl_game(data, http, meta, game, &rules, "nfl").await
            {
                eprintln!(
                    "Failed to process game {} for season {}: {error}",
                    game.id,
                    meta.season.id
                );
            }
        }
    }

    let scored_games = games.iter().filter(|g| final_score(g).is_some()).count();
    Ok((games.len(), scored_games, leaders_line))
}

fn wc_match_result_from_api(season_id: i64, m: &Match) -> Option<WcMatchResult> {
    let (home_goals, away_goals) = crate::soccar::full_time_score(m)?;
    Some(WcMatchResult {
        season_id,
        match_id: m.id,
        home_team_id: m.home_team.id,
        away_team_id: m.away_team.id,
        home_goals,
        away_goals,
        stage: m.stage.clone(),
    })
}

fn nfl_match_result_from_api(season_id: i64, game: &NflGame) -> Option<NflMatchResult> {
    let (home_score, away_score) = final_score(game)?;
    Some(NflMatchResult {
        season_id,
        game_id: game.id,
        home_team_id: game.home_team.id,
        away_team_id: game.away_team.id,
        home_score,
        away_score,
        finished_at: None,
    })
}

async fn process_wc_match(
    data: &Data,
    http: &serenity::Http,
    meta: &SeasonMeta,
    m: &Match,
    rules: &scoring::ScoringRules,
    league_slug: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let season = &meta.season;
    let Some((home_goals, away_goals)) = crate::soccar::full_time_score(m) else {
        return Ok(());
    };

    let updates = {
        let conn = data.db.lock().await;
        if WcProcessedMatch::is_processed(&conn, season.id, m.id)? {
            let stored = WcMatchResult::score(&conn, season.id, m.id)?;
            if stored == Some((home_goals, away_goals)) {
                return Ok(());
            }
            WcProcessedMatch::unmark(&conn, season.id, m.id)?;
        }

        let finished = scoring::FinishedMatch {
            home_team_id: m.home_team.id,
            away_team_id: m.away_team.id,
            home_goals,
            away_goals,
        };

        let mut updates = Vec::new();

        if let Some(registration) =
            Registration::get_by_team(&conn, season.id, m.home_team.id)?
        {
            let points = scoring::points_for_team_in_match(rules, registration.team_id, &finished);
            let total = standings::user_points(&conn, season.id, registration.user_id)?;
            updates.push(MatchUpdate {
                user_id: registration.user_id,
                team_name: registration.team_name,
                points_earned: points,
                total_points: total,
            });
        }

        if let Some(registration) =
            Registration::get_by_team(&conn, season.id, m.away_team.id)?
        {
            let points = scoring::points_for_team_in_match(rules, registration.team_id, &finished);
            let total = standings::user_points(&conn, season.id, registration.user_id)?;
            updates.push(MatchUpdate {
                user_id: registration.user_id,
                team_name: registration.team_name,
                points_earned: points,
                total_points: total,
            });
        }

        if updates.is_empty() {
            WcProcessedMatch::mark(&conn, season.id, m.id)?;
            return Ok(());
        }

        WcProcessedMatch::mark(&conn, season.id, m.id)?;
        updates
    };

    post_match_announcement(
        http,
        season.announce_channel_id,
        &MatchAnnouncement {
            title: &format!("{} — full time", m.stage.as_deref().unwrap_or(&meta.league_name)),
            home_name: &m.home_team.name,
            away_name: &m.away_team.name,
            home_score: home_goals,
            away_score: away_goals,
        },
        &updates,
        rules,
        league_slug,
    )
    .await
}

async fn process_nfl_game(
    data: &Data,
    http: &serenity::Http,
    meta: &SeasonMeta,
    game: &NflGame,
    rules: &scoring::ScoringRules,
    league_slug: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let season = &meta.season;
    let Some((home_score, away_score)) = final_score(game) else {
        return Ok(());
    };

    let updates = {
        let conn = data.db.lock().await;
        if NflProcessedGame::is_processed(&conn, season.id, game.id)? {
            let stored = NflMatchResult::score(&conn, season.id, game.id)?;
            if stored == Some((home_score, away_score)) {
                return Ok(());
            }
            NflProcessedGame::unmark(&conn, season.id, game.id)?;
        }

        let finished = scoring::FinishedMatch {
            home_team_id: game.home_team.id,
            away_team_id: game.away_team.id,
            home_goals: home_score,
            away_goals: away_score,
        };

        let mut updates = Vec::new();

        if let Some(registration) =
            Registration::get_by_team(&conn, season.id, game.home_team.id)?
        {
            let points = scoring::points_for_team_in_match(rules, registration.team_id, &finished);
            let total = standings::user_points(&conn, season.id, registration.user_id)?;
            updates.push(MatchUpdate {
                user_id: registration.user_id,
                team_name: registration.team_name,
                points_earned: points,
                total_points: total,
            });
        }

        if let Some(registration) =
            Registration::get_by_team(&conn, season.id, game.away_team.id)?
        {
            let points = scoring::points_for_team_in_match(rules, registration.team_id, &finished);
            let total = standings::user_points(&conn, season.id, registration.user_id)?;
            updates.push(MatchUpdate {
                user_id: registration.user_id,
                team_name: registration.team_name,
                points_earned: points,
                total_points: total,
            });
        }

        if updates.is_empty() {
            NflProcessedGame::mark(&conn, season.id, game.id)?;
            return Ok(());
        }

        NflProcessedGame::mark(&conn, season.id, game.id)?;
        updates
    };

    post_match_announcement(
        http,
        season.announce_channel_id,
        &MatchAnnouncement {
            title: &format!("{} — final", meta.league_name),
            home_name: &game.home_team.name,
            away_name: &game.away_team.name,
            home_score,
            away_score,
        },
        &updates,
        rules,
        league_slug,
    )
    .await
}

struct MatchAnnouncement<'a> {
    title: &'a str,
    home_name: &'a str,
    away_name: &'a str,
    home_score: i64,
    away_score: i64,
}

async fn post_match_announcement(
    http: &serenity::Http,
    channel_id: Option<u64>,
    announcement: &MatchAnnouncement<'_>,
    updates: &[MatchUpdate],
    rules: &scoring::ScoringRules,
    league_slug: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(channel_id) = channel_id else {
        return Ok(());
    };

    let score_line = format!(
        "{}–{}",
        announcement.home_score, announcement.away_score
    );
    let mut description = format!(
        "**{}** {score_line} **{}**\n\n",
        announcement.home_name, announcement.away_name
    );

    for update in updates {
        let mention = serenity::UserId::new(update.user_id).mention();
        description.push_str(&format!(
            "{mention} ({}) +{} pts → **{}** total\n",
            update.team_name,
            format_points(update.points_earned, league_slug),
            format_points(update.total_points, league_slug),
        ));
    }

    description.push_str(&format!(
        "\nScoring: {}",
        format_rules_footer(rules, league_slug)
    ));

    let embed = serenity::CreateEmbed::default()
        .title(announcement.title)
        .description(description)
        .colour(serenity::Colour::DARK_GREEN);

    serenity::ChannelId::new(channel_id)
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

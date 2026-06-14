use poise::serenity_prelude as serenity;

use crate::{
    db::Season,
    db::NFL_LEAGUE_SLUG,
    standings::{
        self, format_standings_detail_lines, format_standings_summary_lines, standings_footer,
    },
    types::{Context, Error},
};

use super::super::helpers::guild_id;

/// Show the points leaderboard
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn standings(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;
    let (rows, league_slug) = {
        let conn = ctx.data().db.lock().await;
        let season = Season::default_for_guild(&conn, guild_id)?;
        let league_slug = Season::league_slug_for(&conn, season.id)?;
        let rows = standings::get_standings(&conn, season.id)?;
        (rows, league_slug)
    };

    if rows.is_empty() {
        ctx.say("No standings yet — complete a draft with `/draft start` first.")
            .await?;
        return Ok(());
    }

    let title = if league_slug == NFL_LEAGUE_SLUG {
        "NFL standings"
    } else {
        "World Cup standings"
    };

    let footer = standings_footer(&league_slug);
    let ranks = standings::standings_ranks(&rows);
    let summary_lines = format_standings_summary_lines(&rows, &ranks, &league_slug);
    let detail_lines = format_standings_detail_lines(&rows, &ranks, &league_slug);

    let summary_embed = serenity::CreateEmbed::default()
        .title(title)
        .description(summary_lines.join("\n"))
        .footer(serenity::CreateEmbedFooter::new(&footer));

    let reply = ctx
        .send(poise::CreateReply::default().embed(summary_embed))
        .await?;
    let message = reply.into_message().await?;

    let detail_embed = serenity::CreateEmbed::default()
        .title("Standings breakdown")
        .description(detail_lines.join("\n\n"))
        .footer(serenity::CreateEmbedFooter::new(footer));

    let thread = message
        .channel_id
        .create_thread_from_message(
            ctx.serenity_context(),
            message.id,
            serenity::CreateThread::new("Standings breakdown"),
        )
        .await?;

    thread
        .id
        .send_message(
            ctx.serenity_context(),
            serenity::CreateMessage::new().embed(detail_embed),
        )
        .await?;

    Ok(())
}

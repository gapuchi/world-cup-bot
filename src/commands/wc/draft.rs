use poise::serenity_prelude as serenity;

use crate::{draft, types::{Context, Error}};

use super::super::helpers::guild_id;

async fn announce_draft_message(
    ctx: &Context<'_>,
    guild_id: u64,
    content: &str,
) -> Result<(), Error> {
    let channel_id = {
        let conn = ctx.data().db.lock().await;
        let season = crate::db::Season::default_for_guild(&conn, guild_id)?;
        season.announce_channel_id
    };

    if let Some(channel_id) = channel_id {
        serenity::ChannelId::new(channel_id)
            .send_message(
                ctx.serenity_context(),
                serenity::CreateMessage::new().content(content),
            )
            .await?;
    }

    Ok(())
}

fn parse_user_mentions(text: &str) -> Vec<u64> {
    let mut ids = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<@") {
        rest = &rest[start + 2..];
        let end = rest.find('>').unwrap_or(rest.len());
        let id_part = rest[..end].trim_start_matches('!');
        if let Ok(id) = id_part.parse::<u64>() {
            ids.push(id);
        }
        rest = &rest[end..];
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Pick a World Cup nation during your draft turn
#[poise::command(prefix_command, slash_command, guild_only, rename = "pick")]
pub async fn draft_pick(
    ctx: Context<'_>,
    #[description = "World Cup team name, abbreviation, or code (e.g. Brazil, BRA)"] team: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let (message, turn_change) =
        crate::registration::pick_for_user(ctx.data(), guild_id, ctx.author().id.get(), &team)
            .await?;
    ctx.say(&message).await?;

    if let Some(turn_change) = turn_change {
        announce_draft_message(&ctx, guild_id, &draft::format_turn_message(&turn_change)).await?;
    }

    Ok(())
}

/// Start a snake draft with random pick order
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "start",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn draft_start(
    ctx: Context<'_>,
    #[description = "Number of rounds (teams per participant)"] rounds: u32,
    #[description = "Mention draft participants (@user1 @user2 ...)"]
    #[rest]
    participants: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = guild_id(&ctx)?;
    let member_ids = parse_user_mentions(&participants);

    if member_ids.len() < 2 {
        ctx.say("Mention at least two draft participants (e.g. `@alice @bob`).")
            .await?;
        return Ok(());
    }

    match draft::start(ctx.data(), guild_id, member_ids, rounds as i64).await {
        Ok((view, turn_change)) => {
            let order = draft::format_order(&view.participants);
            let start_message = format!(
                "Draft started — **{}** round(s), **{}** pick(s) total.\nPick order: {order}\n\n{}",
                view.rounds,
                view.total_picks,
                draft::format_turn_message(&turn_change)
            );
            ctx.say(&start_message).await?;
            announce_draft_message(&ctx, guild_id, &start_message).await?;
        }
        Err(error) => {
            ctx.say(error.to_string()).await?;
        }
    }

    Ok(())
}

/// Skip the current picker's turn
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "skip",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn draft_skip(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;

    match draft::skip(ctx.data(), guild_id).await {
        Ok(turn_change) => {
            let message = format!("Skipped.\n\n{}", draft::format_turn_message(&turn_change));
            ctx.say(&message).await?;
            announce_draft_message(&ctx, guild_id, &message).await?;
        }
        Err(error) => {
            ctx.say(error.to_string()).await?;
        }
    }

    Ok(())
}

/// Cancel the draft and delete all registrations
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "cancel",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn draft_cancel(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;

    match draft::cancel(ctx.data(), guild_id).await {
        Ok(()) => {
            ctx.say("Draft cancelled. All team assignments for this season were removed.")
                .await?;
        }
        Err(error) => {
            ctx.say(error.to_string()).await?;
        }
    }

    Ok(())
}

/// Show draft status for this season
#[poise::command(prefix_command, slash_command, guild_only, rename = "status")]
pub async fn draft_status(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = guild_id(&ctx)?;

    match draft::status(ctx.data(), guild_id).await? {
        None => {
            ctx.say("No draft for this season. An admin can start one with `/draft start`.")
                .await?;
        }
        Some(view) => {
            let status_label = match view.status {
                crate::db::DraftStatus::Active => "Active",
                crate::db::DraftStatus::Complete => "Complete",
            };
            let order = draft::format_order(&view.participants);
            let mut lines = vec![
                format!("**Status:** {status_label}"),
                format!("**Rounds:** {}", view.rounds),
                format!(
                    "**Progress:** {} / {} picks",
                    view.current_pick.min(view.total_picks),
                    view.total_picks
                ),
                format!("**Pick order:** {order}"),
            ];
            if view.status == crate::db::DraftStatus::Active
                && let Some(picker) = view.current_picker
            {
                lines.push(format!("**On the clock:** <@{picker}>"));
            }
            ctx.say(lines.join("\n")).await?;
        }
    }

    Ok(())
}

/// World Cup draft
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    subcommands(
        "draft_pick",
        "draft_start",
        "draft_skip",
        "draft_cancel",
        "draft_status"
    )
)]
pub async fn draft(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_user_mentions;

    #[test]
    fn parses_discord_mentions() {
        assert_eq!(
            parse_user_mentions("pick @alice and <@123> plus <@!456>"),
            vec![123, 456]
        );
    }
}

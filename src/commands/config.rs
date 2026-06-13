use poise::serenity_prelude as serenity;
use serenity::Mentionable;

use crate::{
    db::{GuildConfig, Pool, Season, SeasonDisplay, league_exists, league_supports_pool},
    types::{Context, Error},
};

use super::helpers::guild_id;

/// Set the active league for commands
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "league",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_league(
    ctx: Context<'_>,
    #[description = "League slug (e.g. wc)"] league: String,
) -> Result<(), Error> {
    let slug = league.trim().to_lowercase();

    {
        let conn = ctx.data().db.lock().await;
        if !league_exists(&conn, &slug)? {
            ctx.say(format!("Unknown league \"{slug}\".")).await?;
            return Ok(());
        }
    }

    if !league_supports_pool(&slug) {
        ctx.say(format!("League \"{slug}\" is not supported yet.")).await?;
        return Ok(());
    }

    let message = {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let Some(pool) = Pool::get_for_guild_league(&conn, guild_id, &slug)? else {
            ctx.say(format!(
                "No pool for \"{slug}\" in this server. Use `/config season` to create one."
            ))
            .await?;
            return Ok(());
        };
        GuildConfig::set_default_pool_id(&conn, guild_id, pool.id)?;
        let display = SeasonDisplay::for_pool(&conn, pool.id)?;
        format!(
            "Active league set to **{}** — tracking **{}** (`{}`).",
            display.league_name, display.name, display.slug
        )
    };

    ctx.say(message).await?;
    Ok(())
}

/// List configured league pools
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "leagues",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_leagues(ctx: Context<'_>) -> Result<(), Error> {
    let (default_pool_id, pools) = {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let default_pool_id = GuildConfig::get(&conn, guild_id)?.map(|config| config.default_pool_id);
        let pools = Pool::list_with_league(&conn, guild_id)?;
        (default_pool_id, pools)
    };

    if pools.is_empty() {
        ctx.say("No league pools configured. Use `/config season` to create one.")
            .await?;
        return Ok(());
    }

    let lines: Vec<String> = pools
        .iter()
        .map(|entry| {
            let active = default_pool_id == Some(entry.pool.id);
            let marker = if active { " (active)" } else { "" };
            format!(
                "**{}** (`{}`){marker}",
                entry.league_name, entry.league_slug
            )
        })
        .collect();

    ctx.say(lines.join("\n")).await?;
    Ok(())
}

/// Set the channel for match result announcements
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "channel",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_channel(
    ctx: Context<'_>,
    #[description = "Channel for match result announcements"] channel: serenity::GuildChannel,
) -> Result<(), Error> {
    let channel_id = channel.id.get();

    {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let pool = Pool::default_for_guild(&conn, guild_id)?;
        Pool::set_announce_channel(&conn, pool.id, channel_id)?;
    }

    ctx.say(format!(
        "Match announcements will be posted in {}.",
        channel.mention()
    ))
    .await?;
    Ok(())
}

/// Create a season and pool for this server
#[poise::command(
    prefix_command,
    slash_command,
    guild_only,
    rename = "season",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config_season(
    ctx: Context<'_>,
    #[description = "League slug (e.g. wc)"] league: String,
    #[description = "Season slug (e.g. wc-2026)"] slug: String,
    #[description = "Display name (e.g. World Cup 2026)"] name: String,
) -> Result<(), Error> {
    let league_slug = league.trim().to_lowercase();
    let season_slug = slug.trim().to_lowercase();
    let season_name = name.trim();

    if season_name.is_empty() {
        ctx.say("Season name is required.").await?;
        return Ok(());
    }

    {
        let conn = ctx.data().db.lock().await;
        if !league_exists(&conn, &league_slug)? {
            ctx.say(format!("Unknown league \"{league_slug}\".")).await?;
            return Ok(());
        }
    }

    if !league_supports_pool(&league_slug) {
        ctx.say(format!("League \"{league_slug}\" is not supported yet.")).await?;
        return Ok(());
    }

    let message = {
        let conn = ctx.data().db.lock().await;
        let guild_id = guild_id(&ctx)?;
        let season = Season::get_or_create(
            &conn,
            &league_slug,
            &season_slug,
            season_name,
        )?;
        let pool = Pool::get_or_create_for_season(&conn, guild_id, season.id)?;
        GuildConfig::set_default_pool_id(&conn, guild_id, pool.id)?;
        let display = SeasonDisplay::for_pool(&conn, pool.id)?;
        format!(
            "Created pool for **{}** — tracking **{}** (`{}`). Set `/config channel` for announcements.",
            display.league_name, display.name, display.slug
        )
    };

    ctx.say(message).await?;
    Ok(())
}

/// Server configuration
#[poise::command(
    prefix_command,
    slash_command,
    subcommands("config_channel", "config_league", "config_leagues", "config_season")
)]
pub async fn config(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

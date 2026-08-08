use crate::types::{Context, Error};

/// Health check
#[poise::command(prefix_command, slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("pong").await?;
    Ok(())
}

/// Show the bot version
#[poise::command(prefix_command, slash_command)]
pub async fn version(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(format!("league-bot **v{}**", env!("CARGO_PKG_VERSION")))
        .await?;
    Ok(())
}

/// Re-register slash commands in this server
#[poise::command(prefix_command, slash_command)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

/// List commands and how to use them
#[poise::command(prefix_command, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    let config = poise::builtins::HelpConfiguration {
        extra_text_at_bottom: "Use `/help <command>` for details on a specific command.",
        include_description: true,
        show_subcommands: true,
        ..Default::default()
    };
    poise::builtins::help(ctx, command.as_deref(), config).await?;
    Ok(())
}

mod api;
mod commands;
mod db;
mod poller;
mod scoring;
mod types;

use poise::serenity_prelude as serenity;
use rusqlite::Connection;
use std::sync::Arc;

use crate::{poller::start_poller, types::Data};

fn database_path() -> String {
    std::env::var("DATABASE_PATH").unwrap_or_else(|_| "world_cup.db".into())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let token =
        std::env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in the environment");
    let api_token = std::env::var("FOOTBALL_DATA_API_TOKEN")
        .expect("Expected FOOTBALL_DATA_API_TOKEN in the environment");

    let db_path = database_path();
    let conn = Connection::open(&db_path).unwrap_or_else(|error| {
        panic!("Failed to open database at {db_path}: {error}");
    });
    db::init(&conn).expect("Failed to initialize database");

    let data = Data {
        db: Arc::new(tokio::sync::Mutex::new(conn)),
        http: reqwest::Client::new(),
        api_token,
    };

    let intents = serenity::GatewayIntents::GUILDS;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("$".into()),
                case_insensitive_commands: true,
                ..Default::default()
            },
            commands: vec![
                commands::ping(),
                commands::help(),
                commands::register(),
                commands::config(),
                commands::claim(),
                commands::assign(),
                commands::unclaim(),
                commands::my_team(),
                commands::teams(),
                commands::unclaimed(),
                commands::standings(),
                commands::pick_player(),
                commands::season(),
            ],
            ..Default::default()
        })
        .setup({
            let bot_data = data.clone();
            move |ctx, ready, framework| {
                let bot_data = bot_data.clone();
                Box::pin(async move {
                    let commands = &framework.options().commands;
                    for guild in &ready.guilds {
                        if let Err(error) = poise::builtins::register_in_guild(
                            &ctx.http,
                            commands,
                            guild.id,
                        )
                        .await
                        {
                            eprintln!(
                                "Failed to register slash commands in guild {}: {error}",
                                guild.id
                            );
                        }
                    }

                    start_poller(Arc::new(bot_data.clone()), ctx.http.clone());
                    Ok(bot_data)
                })
            }
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    match client {
        Ok(mut client) => {
            if let Err(error) = client.start().await {
                eprintln!("Error starting client: {error}");
            }
        }
        Err(error) => {
            eprintln!("Error creating client: {error}");
        }
    }
}

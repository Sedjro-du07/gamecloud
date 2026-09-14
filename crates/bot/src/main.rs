//! GameCloud OS Discord bot entry point.

#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::doc_markdown)]
#![allow(clippy::needless_raw_string_hashes)]
// Hex color codes (e.g. `0x9c4dff`) are more readable as a single
// chunk than `0x9c_4dff`.
#![allow(clippy::unreadable_literal)]
// `body.push_str(&format!(...))` reads more naturally than
// `write!(&mut body, ...)` for short Discord-message builders.
#![allow(clippy::format_push_string)]
// `Vec<(String, ...)>` row tuples in command handlers are not worth
// extracting into named types.
#![allow(clippy::type_complexity)]

use std::time::Duration;

use poise::serenity_prelude as serenity;
use serenity::FullEvent;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

mod commands;
mod config;
mod events;
mod state;

use crate::{config::Config, state::BotState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,serenity=warn")),
        )
        .json()
        .init();

    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(30))
        .connect(&config.database_url)
        .await?;

    let state = BotState::new(config.clone(), pool);

    let intents = serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_MEMBERS;

    let framework = poise::Framework::<BotState, anyhow::Error>::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            event_handler: |ctx, event, _framework, data| {
                Box::pin(handle_event(ctx, event, data.clone()))
            },
            on_error: |error| Box::pin(async move {
                tracing::error!(error = ?error, "command error");
            }),
            ..Default::default()
        })
        .setup(move |ctx, ready, framework| {
            let state_for_setup = state.clone();
            Box::pin(async move {
                tracing::info!(user = %ready.user.name, "bot ready");
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                events::outbox::spawn(state_for_setup.clone(), ctx.http.clone());
                Ok(state_for_setup)
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(&config.discord_token, intents)
        .framework(framework)
        .await?;

    client.start().await?;
    Ok(())
}

async fn handle_event(
    ctx: &serenity::Context,
    event: &FullEvent,
    state: BotState,
) -> Result<(), anyhow::Error> {
    if let FullEvent::Message { new_message } = event {
        events::draftbot::on_message_create(ctx, new_message, &state).await;
    }
    Ok(())
}

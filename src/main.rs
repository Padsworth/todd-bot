// main.rs

use dotenv::dotenv;
use poise::serenity_prelude as serenity;
use serenity::prelude::TypeMapKey;
use std::{env::var, sync::Arc, time::Duration};
use tokio::sync::Mutex;
mod calendar;
mod databaser;
mod errors;
mod helper;
mod models;
mod schema;
mod shitposts;
mod todd_commands;
use crate::models::*;

pub struct Data {
    pub reminders: Arc<Mutex<Vec<Reminder>>>,
} // User data, which is stored and accessible
  // in all command invocations
  // Types used by all command functions
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;
pub struct RemindersKey;
impl TypeMapKey for RemindersKey {
    type Value = Arc<Mutex<Vec<Reminder>>>;
}

#[tokio::main]
async fn main() {
    // env_logger::init();

    dotenv().ok();
    // FrameworkOptions contains all of poise's configuration option in one struct
    // Every option can be omitted to use its default value
    let options = poise::FrameworkOptions {
        commands: vec![
            todd_commands::add(),
            todd_commands::todd(),
            todd_commands::old_quotes(),
            shitposts::nerd(),
            calendar::calendar(),
        ],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some("!".into()),
            edit_tracker: Some(poise::EditTracker::for_timespan(Duration::from_secs(3600))),
            additional_prefixes: vec![
                poise::Prefix::Literal("Todd!"),
                poise::Prefix::Literal("todd!"),
            ],
            ..Default::default()
        },
        // The global error handler for all error cases that may occur
        on_error: |error| Box::pin(errors::on_error(error)),
        // This code is run before every command
        pre_command: |ctx| {
            Box::pin(async move {
                println!(
                    "{} is executing command {}...",
                    ctx.author().name,
                    ctx.command().qualified_name
                );
            })
        },
        // This code is run after a command if it was successful (returned Ok)
        post_command: |ctx| {
            Box::pin(async move {
                println!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        // Every command invocation must pass this check to continue execution
        command_check: Some(|ctx| {
            Box::pin(async move {
                let bot_id = var("BOT_ID")
                    .expect("Missing BOT_ID")
                    .parse::<u64>()
                    .expect("Invalid BOT_ID");
                if ctx.author().id == bot_id {
                    return Ok(false);
                }
                Ok(true)
            })
        }),

        // Enforce command checks even for owners (enforced by default)
        // Set to true to bypass checks, which is useful for testing
        skip_checks_for_owners: false,
        // event_handler: |_ctx, event, _framework, _data| {
        //     Box::pin(async move {
        //         println!("Got an event in event handler: {:?}", event.name());
        //         Ok(())
        //     })
        // },
        ..Default::default()
    };

    let token = "DISCORD_TOKEN";
    poise::Framework::builder()
        .token(var(token).expect("missing DISCORD_TOKEN"))
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", _ready.user.name);
                let reminders = Arc::new(Mutex::new(Vec::new()));
                let data = Data {
                    reminders: reminders.clone(),
                };
                ctx.data.write().await.insert::<RemindersKey>(reminders);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                tokio::spawn(calendar::check_events_loop(ctx.clone()));
                tokio::spawn(calendar::fetch_events_loop(ctx.clone()));
                Ok(data)
            })
        })
        .options(options)
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .run()
        .await
        .unwrap();
}

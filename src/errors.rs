// errors.rs

use crate::{Context, Data, Error};

pub async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);
            error_reply(ctx, error).await;
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}

async fn error_reply(ctx: Context<'_>, error: Error) {
    let reply = format!(
        "Error in command `{}`: {:?}\n**{}**",
        ctx.command().name,
        error,
        error
    );

    let _ = ctx.reply(reply).await;
}

pub async fn message_too_large_handler(ctx: Context<'_>, input: &str) -> Result<(), Error> {
    let (first_half, second_half) = split_in_half(input);

    ctx.reply(first_half).await?;
    ctx.say(second_half).await?;

    Ok(())
}
fn split_in_half(input: &str) -> (&str, &str) {
    let mid = input.len() / 2;
    let (left, right) = input.split_at(mid);
    (left, right)
}

// shitpost.rs

use crate::{Context, Error};
// use poise::serenity_prelude as serenity;

#[poise::command(
    prefix_command,
    global_cooldown = 30,
    category = "Shitpost",
    broadcast_typing,
    member_cooldown = 600
)]
pub async fn nerd(ctx: Context<'_>) -> Result<(), Error> {
    reply_to_reply(ctx, "data/gifs/nerd-emoji.gif").await?;

    Ok(())
}
#[poise::command(
    prefix_command,
    global_cooldown = 30,
    category = "Shitpost",
    broadcast_typing,
    member_cooldown = 600
)]
pub async fn crumble(ctx: Context<'_>) -> Result<(), Error> {
    reply_to_reply(ctx, "data/gifs/nerd-emoji.gif").await?;

    Ok(())
}

async fn reply_to_reply(ctx: Context<'_>, file_location: &str) -> Result<(), Error> {
    let message = if let Context::Prefix(p) = ctx {
        p.msg
    } else {
        return Err(Error::from("Error: not prefix command"));
    };
    if let Some(message_reply) = &message.referenced_message {
        ctx.channel_id()
            .send_message(&ctx.http(), |m| {
                m.add_file(file_location)
                    .reference_message(&**message_reply)
            })
            .await?;
    } else {
        return Err(Error::from("Error: no referenced message"));
    }

    Ok(())
}

#[poise::command(
    prefix_command,
    global_cooldown = 30,
    category = "Shitpost",
    broadcast_typing,
    // subcommands("add", "remove"),
    // subcommand_required,
    // help_text_fn = "CommandHelp::Add.help()",
)]
pub async fn sp(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

// help.rs

use crate::{Context, Error};

pub enum CommandHelp {
    AddQuote,
    AddMember,
    ToddBot,
}

impl CommandHelp {
    pub fn help(&self) -> &'static str {
        match self {
            CommandHelp::AddMember => "Add a new member who can recieve quotes",
            CommandHelp::AddQuote => "Add a new quote to a member's quote list",
            CommandHelp::ToddBot => "Get a random quote!",
        }
    }
}

pub async fn help(ctx: Context<'_>, command: Option<String>) -> Result<(), Error> {
    let config = poise::builtins::HelpConfiguration {
        extra_text_at_bottom: "\
        Type `!help command` for more info on a command.",
        ..Default::default()
    };
    poise::builtins::help(ctx, command.as_deref(), config).await?;
    Ok(())
}

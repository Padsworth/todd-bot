// todd_commands.rs

use crate::databaser;
use crate::errors;
use crate::models::SchlonghouseMember;
// use crate::helper::CommandHelp;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use serenity::SerenityError;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};

#[poise::command(
    prefix_command,
    global_cooldown = 30,
    category = "Based Todd",
    broadcast_typing,
    subcommands("quote", "member", "nickname"),
    subcommand_required,
    // help_text_fn = "CommandHelp::Add.help()",
)]
pub async fn add(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}
#[poise::command(
    prefix_command,
    global_cooldown = 30,
    // member_cooldown = 300,
    // help_text_fn = "help::CommandHelp::Add.help()",
)]
pub async fn quote(ctx: Context<'_>, input: String, #[rest] message: String) -> Result<(), Error> {
    let member_id = parse_member_or_return_lowercase(&input);

    let mut conn = databaser::establish_connection()?;
    let schlonghouse_member = databaser::get_member(&mut conn, &member_id)?;
    let member_primary_name = schlonghouse_member.primary_name;
    let schlong_id = schlonghouse_member.id;
    let member_quote_file = format!("data/{}.quotes.txt", member_primary_name);

    databaser::create_quote(&mut conn, &member_primary_name, &message)?;

    let contents = format!("{}\n", message);
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(member_quote_file)?;

    file.write_all(contents.as_bytes())?;

    let sender = ctx.author();
    let response = format!(
        "{} added message\n**{}**\nto {}'s quotes list\n<@{}>",
        sender, message, member_primary_name, schlong_id
    );

    ctx.reply(response).await?;

    Ok(())
}
#[poise::command(prefix_command, global_cooldown = 30, broadcast_typing)]
pub async fn nickname(ctx: Context<'_>, member: String, nickname: String) -> Result<(), Error> {
    let member_id = parse_member_or_return_lowercase(&member);
    let mut conn = databaser::establish_connection()?;
    let schlonghouse_member = databaser::get_member(&mut conn, &member_id)?;

    let lowercase_nickname = nickname.to_lowercase();
    let created_nickname =
        databaser::create_nickname(&mut conn, &schlonghouse_member, &lowercase_nickname)?;

    let resp = format!(
        "{} added a new nickname: **{}** to <@{}>'s nicknames. \nYou can now refer to them as {} in any commands",
        ctx.author(), created_nickname.nickname, schlonghouse_member.id, nickname
    );

    ctx.reply(resp).await?;

    Ok(())
}

#[poise::command(
    prefix_command,
    global_cooldown = 60,
    member_cooldown = 300,
    category = "Based Todd",
    broadcast_typing
)]
pub async fn todd(ctx: Context<'_>, input: String) -> Result<(), Error> {
    let member_id = parse_member_or_return_lowercase(&input);
    let mut conn = databaser::establish_connection()?;

    // Complicated mess that basically returns a SchlonghouseMember based on
    // a few queries, trying first the id, then querying based on primary name
    // finally trying nickname
    let schlonghouse_member = databaser::get_member(&mut conn, &member_id)?;

    let all_quotes =
        databaser::get_all_members_quotes(&mut conn, &schlonghouse_member.primary_name)?;
    let random_quote = databaser::get_random_quote_from_quotes(all_quotes)?;
    let response = format!("\"{}\"", random_quote);
    ctx.reply(response).await?;

    Ok(())
}

#[poise::command(prefix_command, global_cooldown = 60/*, member_cooldown = 300*/)]
pub async fn member(ctx: Context<'_>, primary_name: String, id: String) -> Result<(), Error> {
    let member_id = if let Some(member_at) = serenity::utils::parse_username(&id) {
        member_at.to_string()
    } else {
        id.clone()
    };

    let checked_member: bool = serenity::utils::parse_username(&id).is_some();

    let member_id_parsed = member_id.parse::<i64>()?;
    let mut conn = databaser::establish_connection()?;

    let created_member =
        databaser::create_member(&mut conn, member_id_parsed, &primary_name, checked_member)?;

    let author = ctx.author();
    let resp = format!(
        "{} added a new member:\n**{}**\n\nto add nicknames for this member use:\n`!add nickname {} <new_nickname>`",
        author,
        created_member.primary_name,
        created_member.primary_name
    );

    ctx.reply(resp).await?;

    Ok(())
}

pub fn parse_member_or_return_lowercase(value: &String) -> String {
    if let Some(parsed_at) = serenity::utils::parse_username(value) {
        parsed_at.to_string()
    } else {
        value.to_lowercase()
    }
}

fn retrieve_old_quotes(person: &SchlonghouseMember) -> Result<Vec<String>, Error> {
    let file_path = format!(
        "/home/paddy/todd/cogs/files/roasts/{}.txt",
        person.primary_name
    );
    println!("looking for quotes file at:\n{}", &file_path);
    let file = OpenOptions::new().read(true).open(file_path)?;
    let reader = BufReader::new(file);
    let output: Vec<String> = reader.lines().map(|line| line.unwrap()).collect();

    Ok(output)
}

#[poise::command(
    prefix_command,
    global_cooldown = 60,
    member_cooldown = 86400,
    category = "Based Todd",
    broadcast_typing
)]
pub async fn old_quotes(ctx: Context<'_>, input: String) -> Result<(), Error> {
    let member_id = parse_member_or_return_lowercase(&input);
    let mut conn = databaser::establish_connection()?;
    let schlonghouse_member = databaser::get_member(&mut conn, &member_id)?;

    let old_quotes_vector = retrieve_old_quotes(&schlonghouse_member)?;
    let old_quotes_string = old_quotes_vector.join("\n");
    let response = format!(
        "{}'s old quotes are:\n{}",
        schlonghouse_member.primary_name, old_quotes_string
    );

    let ctx_resp = ctx.reply(&response).await;

    if let Err(SerenityError::Model(_)) = ctx_resp {
        errors::message_too_large_handler(ctx, &response).await?
    };

    Ok(())
}

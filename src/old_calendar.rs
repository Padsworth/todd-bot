// calendar.rs
use crate::{Context, Error};
use crate::models::{NewEvent, ToddEvent, NewReminder, Reminder,
                    BirthdayEvent, CalendarType, ToCalendar};
use chrono::prelude::*;
use diesel::prelude::*;
use diesel::pg::PgConnection;
use crate::todd_commands;
use crate::databaser;


// remember that connections are established via:
// databaser::establish_connection();

#[poise::command(
    prefix_command,
    member_cooldown = 30,
    category = "Calendar",
    broadcast_typing,
    subcommands("add", "remove"),
    subcommand_required,
)]
pub async fn calendar(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}
#[poise::command(
    prefix_command,
    member_cooldown = 30,
)]
async fn add(ctx: Context<'_>, #[rest] input: String) -> Result<(), Error> {
    let mut conn = databaser::establish_connection()?;
    let split_input = if let Some(s) = input.split_once(' ') {
        s
    } else {
        return Err(Error::from("Error: more arguments needed"))
    };
    let prefixs: Vec<&str> = vec!["event", "birthday", "reminder"];

    let created_event: CalendarType = if let Ok(t) = if split_input.0 == prefixs[0] {
        add_event(&mut conn, ctx, split_input.1)
    } else if split_input.0 == prefixs[1] {
        add_birthday(&mut conn, split_input.1)
    } else if split_input.0 == prefixs[2] {
        add_reminder(&mut conn, split_input.1)
    } else {
        return Err(Error::from("Error: must specify `event`, `reminder`, or `birthday`"))
    } {t.to_calendar()} else {
        return Err(Error::from("Error: must specify `event`, `reminder`, or `birthday`"))
    };

    ctx.say(format!(
        "{} has created new {}: *{}*
{}
It takes place: {}
{}",
        ctx.author(), split_input.0, created_event.title(),
        if let Some(d) = created_event.description() {d} else { "".to_string() },
        created_event.when().format("*%D* at *%I:%M %P*"),
        match created_event {
            CalendarType::Tevent(t) => {
                "It is".to_string() + if t.is_recuring {
                    "recuring"
                } else {
                    "not recuring"
                }
            },
            CalendarType::Teminder(_) => "".to_string(),
        }
    )).await?;

    Ok(())
}
async fn add_event(
    conn: &mut PgConnection,
    ctx: Context<'_>,
    input: &str,
) -> Result<ToddEvent, Error> {

    let new_event = parse_string_into_event(input, ctx)?;
    let created_event = databaser::create_event(
        conn,
        new_event.title,
        new_event.description,
        new_event.timedate,
        new_event.is_recuring,
        new_event.owned_by
    )?;

    let message = format!(
        "{} has created new event: *{}*
{}
It takes place: {}
It is {}",
        ctx.author(), created_event.title,
        new_event.description,
        created_event.timedate.format("*%D* at *%I:%M %P*"),
        if created_event.is_recuring {"recuring"} else {"not recuring"}
    );
    ctx.say(message).await?;
    Ok(created_event)
}

fn add_birthday(conn: &mut PgConnection, input: &str) -> Result<ToddEvent, Error> {
    let new_event = parse_string_into_birthday_event(input)?;
    let created_event = databaser::create_event(
        conn,
        &new_event.title,
        &new_event.description,
        new_event.date,
        new_event.is_recuring,
        new_event.member_id
    )?;
    Ok(created_event)
}
fn add_reminder(conn: PgConnection, input: &str) -> Result<Reminder, Error> {
    let new_reminder = parse_string_into_reminders(input)?;
    // let created_reminder = databser::create_reminder(
    //     conn,
    //     new_time_before: NaiveDateTime,
    //     owned_by_event_id: i32
    // );
    // Ok(created_reminder)
}

#[poise::command(
    prefix_command,
    member_cooldown = 30,
)]
async fn remove(ctx: Context<'_>, #[rest] input: String) -> Result<(), Error> {
    // TODO: pop the first word from the string.
    // see if the word is `event`, `birthday`, or `reminder`
    // then handle each case
    Ok(())
}

fn parse_string_into_event<'a>( input: &'a str, ctx: Context<'_>) -> Result<NewEvent<'a>, Error> {
    let matches: &[_] = &[' ', '=', '\"'];
    let recuring = input.contains("recuring");
    let input_description = if let Some(s) = input.to_lowercase().find("description") {
        let description_output = if let Some(a)= input
            .split_at(s)
            .1
            .trim_start_matches(matches)
            .split_once('\"') {
                a.0.trim()
            } else {
                return Err(Error::from("Formating Error when parsing `description`"))
            };
        description_output
    } else {
        ""
    };
    let input_title = parse_string_into_title(input)?;
    let input_timedate = parse_string_into_timedate(input)?;
    let author_id = ctx.author().id.as_u64();
    let output = NewEvent {
        title: input_title,
        description: input_description,
        timedate: input_timedate,
        is_recuring: recuring,
        owned_by: *author_id as i64
    };
    Ok(output)
}
fn parse_string_into_timedate(input: &str) -> Result<NaiveDateTime, Error> {
    let matches: &[_] = &[' ', '=', '\"'];
    let when = if let Some(s) = input.to_lowercase().find("time") {
        let when_output = if let Some(w) = input
            .split_at(s)
            .1
            .trim_start_matches(matches)
            .trim()
            .split_once('\"') {
                w.0.trim()
            } else {
                return Err(Error::from("Formating Error when parsing `time`"))
            };
        when_output
    } else {
        return Err(Error::from(
            "Must specify time in a quoted `mm/dd/yy hh:mi am/pm` format"
        ))
    };
    let formats_to_try: Vec<&str> = vec![
        "%D %I:%M %P",
        "%D %I:%M %p",
        "%D %R",
        "%D",
    ];
    for f in formats_to_try {
        if let Ok(d) = NaiveDateTime::parse_from_str(when, f) {
            return Ok(d)
        }
    }
    Err(Error::from("Failed to parse input into a datetime"))
}
fn parse_string_into_birthday_event(input: &str) -> Result<BirthdayEvent, Error> {
    let mut conn = databaser::establish_connection()?;
    let split_input = if let Some(s) = input.split_once(' ') {
        s
    } else {
        return Err(Error::from("Input must be `!calendar add birthday person date`"))
    };
    let parsed_member = todd_commands::parse_member_or_return_lowercase(
        &String::from(split_input.0)
    );
    let owner_member = databaser::get_member(& mut conn, &parsed_member)?;
    let input_description = format!(
        "#Its {}'s birthday today!\nHappy birthday <@{}>",
        owner_member.primary_name,
        owner_member.id
    );
    let input_timedate = parse_string_into_timedate(input)?;
    let output = BirthdayEvent {
        title: String::from("Birthday"),
        description: input_description,
        date: input_timedate,
        is_recuring: true,
        member_id: owner_member.id as i64
    };
    Ok(output)
}
fn parse_string_into_title(input: &str) -> Result<&str, Error> {
    let matches: &[_] = &[' ', '=', '\"'];
    if let Some(t) = input
        .trim()
        .trim_start_matches(matches)
        .split_once('\"') {
            Ok(t.0)
        } else {
            Err(Error::from("Title must be quoted"))
        }
}
fn parse_string_into_reminders<'a>(input: &str) -> Result<Vec<NewReminder<'a>>, Error> {
    // TODO: figure out desired reminder input
    // !add reminder for "event" n time_units before
    // !add reminder for "event" timedate
    // NOTE code is currently returning a vec of NewReminders
    // instead, if `events` is a vector greater than 1, return
    // an error containing the id's of all retrieved Events
    // and tell the user to use the id instead
    // NOTE make the code work for id's and titles
    let input = if let Some(s) = input.trim().strip_prefix("for") {
        s.trim()
    } else {input.trim()} ;
    let quotes: &[_] = &['\'', '\"', '`'];
    let split_input = |s: &str| -> Result<(&str, &str), Error> {
        for q in quotes {
            if let Some(t) = s.trim_start_matches(quotes)
                .split_once(*q) { return Ok(t) }
        }
        return Err(Error::from("Must quote title name"))
    };
    let output = if let Ok(s_i) = split_input(input) {
        let mut conn = databaser::establish_connection()?;
        let events = databaser::get_event_by_title(&mut conn, s_i.0)?;
        let new_timebefore = if let Ok(n) = parse_string_into_timedate(s_i.1) {n} else {
            let unbefored = if let Some(b) = s_i.1.strip_prefix("before") {
                b
            } else { s_i.1 };
            let (stime, units) = if let Some(u) = unbefored.trim().split_once(' ') {
                u
            } else { return Err(Error::from("Please Specify time units before")) };
            let ttime = if let Some(t) = stime.parse::<i64> {t} else {
                return Err(Error::from("Must specify an intiger of time before"))
            };
            let vtime = vec![ {
                for e in events {
                    if let Some(t) = NaiveDateTime::from_timestamp_opt(
                        e.timedate.timestamp() - { ttime * {
                            if units == "seconds" {
                                1
                            } else if units == "minutes" {
                                60
                            } else if units == "hours" {
                                60 * 60
                            } else if units == "days" {
                                60 * 60 * 24
                            } else if units == "weeks" {
                                60 * 60 * 24 * 7
                            } else if units == "months" {
                                60 * 60 * 24 * 30
                            } else if units == "years" {
                                60 * 60 * 24 * 7 * 52
                            } else {
                                return Err(Error::from("Must specify unit of time"))
                            }
                    } }, 0) {
                        t
                    } else {
                        return Err(Error::from("Time outside Timestamp Range"))
                    }
                }.collect()
            }];
        };
    };
}

// TODO: fn remove_event
// TODO: fn remove_reminder
// TODO: fn check_upcoming_reminders
// TODO: fn check_upcoming_events
// TODO: have reminders get deleted after they are sent
// also have events that are not recuring delete after they pass

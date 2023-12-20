// calendar.rs
use crate::databaser;
use crate::models::{CalendarType, Reminder, ToCalendar, ToddEvent};
use crate::todd_commands;
use crate::{Context, Error, RemindersKey};
use chrono::prelude::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use poise::serenity_prelude as serenity;
// use std::sync::{Arc};
use tokio::time::{interval, Duration};
// use tokio::sync::{Mutex};
use serenity::{ChannelId, MessageBuilder};
use std::env::var;

pub async fn fetch_reminders() -> Result<Vec<Reminder>, Error> {
    use crate::schema::reminders::dsl::*;

    let mut conn = databaser::establish_connection()?;
    let now = Local::now().naive_local();
    let output = reminders
        .filter(time_before.between(now, now + chrono::Duration::minutes(30)))
        .load::<Reminder>(&mut conn)?;
    Ok(output)
}
pub async fn fetch_events_loop(ctx: serenity::Context) {
    let mut interval = interval(Duration::from_secs(1800));
    loop {
        let fetched_reminders = match fetch_reminders().await {
            Ok(r) => r,
            Err(err) => {
                eprintln!("Failed to fetch events: {}", err);
                continue;
            }
        };
        let reminders_to_watch = ctx
            .data
            .read()
            .await
            .get::<RemindersKey>()
            .cloned()
            .unwrap_or_default();

        let mut locked_reminders = reminders_to_watch.lock().await;

        *locked_reminders = fetched_reminders;
        drop(locked_reminders);
        interval.tick().await;
    }
}

pub fn is_within_one_minute(datetime: NaiveDateTime) -> bool {
    let current_time = Local::now().naive_local();
    let one_minute_later = current_time + chrono::Duration::minutes(1);

    datetime >= current_time && datetime < one_minute_later
}

pub async fn check_events_loop(ctx: serenity::Context) {
    let mut interval = interval(Duration::from_secs(60));
    loop {
        // Error handling in loop is important
        // This is a spot where errors will not reach userland
        // and errors cannot be propigated
        let mut conn = match databaser::establish_connection() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Could not establish conn: {}", e);
                continue;
            }
        };
        // The created  reminders
        let reminders_to_watch = ctx
            .data
            .read()
            .await
            .get::<RemindersKey>()
            .cloned()
            .unwrap_or_default();

        for r in reminders_to_watch.lock().await.clone() {
            if is_within_one_minute(r.time_before) {
                let mut desc_vec = "".to_string();
                // initialized to be modified later
                let mut event = ToddEvent {
                    id: 0,
                    title: "Reminder".to_string(),
                    description: None,
                    timedate: Local::now().naive_local(),
                    is_recuring: false,
                    owned_by: 0,
                    recurring_by: None,
                };
                // try to get reminder's parent. if parent is not found
                // something has gone wrong.
                let parent = {
                    let p = r.parent(&mut conn);
                    handle_parentsome(&mut conn, p, r.clone())
                };
                if let Err(err) = parent {
                    desc_vec.push_str(format!("\n{:?}", err).as_str());
                } else {
                    // Turn event into parent
                    let parent = parent.unwrap();
                    event.id = parent.id;
                    event.title = parent.title;
                    if let Some(d) = parent.description {
                        desc_vec.push_str(format!("\n{}", d).as_str())
                    }
                    event.timedate = parent.timedate;
                    event.owned_by = parent.owned_by;
                    event.recurring_by = parent.recurring_by;
                }
                if !desc_vec.is_empty() {
                    event.description = Some(desc_vec.to_string())
                }

                // if all is well, send the message to userland.
                // The message might have errors in it, but the core should be
                // there.
                if let Err(err) = send_event_message(&ctx, event).await {
                    eprintln!("Error sending reminder message for reminder: {:?}", r);
                    eprintln!("Error: {}", err)
                }
            }
        }
        interval.tick().await;
    }
}
async fn send_event_message(ctx: &serenity::Context, event: ToddEvent) -> Result<(), Error> {
    let message = MessageBuilder::new()
        .push("# ")
        .push(event.title)
        .push("\n## ")
        .push(event.timedate)
        .push("\n")
        .push(match event.description {
            Some(s) => s,
            None => "".to_string(),
        })
        .build();
    // Poise's old version of serenity doesn not have the ChannelId.new() method
    let channel = ChannelId(var("DEFAULT_CHANNEL")?.parse::<u64>()?);
    channel.say(&ctx.http, message).await?;
    Ok(())
}
fn handle_parentsome(
    conn: &mut PgConnection,
    parent: Option<ToddEvent>,
    child: Reminder,
) -> Result<ToddEvent, Error> {
    let mut desc = None;
    let mut desc_vec: String = "".to_string();
    if parent.is_none() {
        return Err(Error::from("Warning: reminder has no parent."));
    } else if parent.clone().unwrap().id != child.event_id {
        return Err(Error::from(format!(
            "Error: parent does not own child. Parent is: {:?}",
            parent
        )));
    }

    let mut parent = parent.unwrap().clone();
    if let Err(err) = databaser::delete_reminder_by_id(conn, child.id) {
        desc_vec.push_str(
            format!(
                "\nError removing reminder: {:?}\n with Error message: {}
Make sure to manually remove the reminder later",
                child, err
            )
            .as_str(),
        )
    }
    if !parent.is_recuring {
        if let Err(err) = databaser::delete_event_by_id(conn, parent.id) {
            desc_vec.push_str("\nWarning: event not recuring but was not deleted");
            desc_vec.push_str(format!("\ndeletion failure casued by error: {:?}", err).as_str());
            desc_vec.push_str("\nEvent may need to be modified/deleted manually")
        }
    } else {
        let handled_recurrance = handle_recurrance(conn, parent.clone());
        if let Err(err) = handled_recurrance {
            desc_vec.push_str("\nWarning: **recuring reminder failed to set**");
            desc_vec.push_str(format!("Err msg: {:?}", err).as_str());
        } else if handled_recurrance.as_ref().unwrap().is_none() {
            desc_vec.push_str("\n**Warning**:");
            desc_vec
                .push_str(format!("{} is both recurring and not recurring", parent.title).as_str())
        } else {
            desc_vec.push_str(
                format!(
                    "\nReminder for recurring event {} set:\n{:?}",
                    parent.title,
                    handled_recurrance.unwrap().unwrap()
                )
                .as_str(),
            )
        }
    }
    if !desc_vec.is_empty() {
        desc = Some(desc_vec)
    }
    parent.description = desc;

    Ok(parent)
}
// remember that connections are established via:
// databaser::establish_connection();
fn handle_recurrance(conn: &mut PgConnection, event: ToddEvent) -> Result<Option<Reminder>, Error> {
    if !event.is_recuring {
        return Ok(None);
    }
    let output = match event.recurring_by {
        Some(0) => {
            let r = create_daily_reminder(conn, event)?;
            Some(r)
        }
        Some(1) => {
            let r = create_weekly_reminder(conn, event)?;
            Some(r)
        }
        Some(2) => {
            let r = create_monthly_reminder(conn, event)?;
            Some(r)
        }
        Some(3) => {
            let r = create_yearly_reminder(conn, event)?;
            Some(r)
        }
        None => {
            return Err(Error::from(
                "Warning: Event is set as recurring but does not have a timeframe",
            ))
        }
        _ => {
            return Err(Error::from(format!(
                "Warning: `recurring_by` set to invalid value: {:?}",
                event.recurring_by
            )))
        }
    };
    Ok(output)
}

#[poise::command(
    prefix_command,
    // member_cooldown = 30,
    category = "Calendar",
    broadcast_typing,
    subcommands("add", "remove", "list"),
    subcommand_required
)]
pub async fn calendar(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}
#[poise::command(prefix_command, member_cooldown = 30)]
// This has 10 args cause i really have no idea how else to handle what I want
// `add` takes a vector of Strings, however when userspace calls the command idk
// if it will make it a vector propperly...
#[allow(clippy::too_many_arguments)]
async fn add(
    ctx: Context<'_>,
    event_type: String,
    arg1: Option<String>,
    arg2: Option<String>,
    arg3: Option<String>,
    arg4: Option<String>,
    arg5: Option<String>,
    arg6: Option<String>,
    arg7: Option<String>,
    arg8: Option<String>,
) -> Result<(), Error> {
    let mut conn = databaser::establish_connection()?;
    let mut args = vec![];
    let option_args = vec![arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8];
    for a in option_args.into_iter().flatten() {
        args.push(a)
    }
    let mut created_event: CalendarType = match event_type.to_lowercase().as_str() {
        "event" => add_event(&mut conn, ctx, args)?,
        "birthday" => add_birthday(&mut conn, args)?.to_calendar(),
        "reminder" => add_reminder(&mut conn, args)?.to_calendar(),
        _ => {
            return Err(Error::from(
                "Error parsing type, specify `event` `birthday` or `reminder`",
            ))
        }
    };
    if let CalendarType::Tevent(t) = created_event.clone() {
        let _reminder = databaser::create_reminder(&mut conn, t.timedate, t.id)?;
        // handling recurrance:
        let mut new_desc = "".to_string();
        let recur = handle_recurrance(&mut conn, t.clone());
        if let Err(err) = recur {
            new_desc.push_str(
                format!(
                    "
\n*Error when handling recurrance: **{:?}**

Event was still created, but reminders might need to be added manually",
                    err
                )
                .as_str(),
            );
        } else if let Some(r) = recur.unwrap() {
            new_desc.push_str(
                format!(
                    "\nRecurring reminder set for: {:?}
recurring on interval: {}
with id: {}",
                    r.time_before,
                    t.recurring_by.unwrap(),
                    r.id
                )
                .as_str(),
            )
        } else {
            new_desc.push_str("\nNo recurring reminder set")
        }
        let old_desc = if let Some(s) = t.description {
            s
        } else {
            "".to_string()
        };
        created_event = {
            ToddEvent {
                id: t.id,
                title: t.title,
                description: Some(old_desc + &new_desc),
                timedate: t.timedate,
                is_recuring: t.is_recuring,
                owned_by: t.owned_by,
                recurring_by: t.recurring_by,
            }
            .to_calendar()
        }
    }

    ctx.say(format!(
        "{} has created new {}: **{}**
{}
It takes place: {}
",
        ctx.author(),
        event_type,
        created_event.title(),
        created_event.description().unwrap_or_default(),
        created_event.when().format("*%D* at *%I:%M %P*"),
    ))
    .await?;

    Ok(())
}
fn add_event(
    conn: &mut PgConnection,
    ctx: Context<'_>,
    input: Vec<String>,
) -> Result<CalendarType, Error> {
    let new_event = parse_event_args(input.clone(), ctx)?;
    let mut created_event = databaser::create_event(
        conn,
        &new_event.title,
        &new_event.description,
        new_event.timedate,
        new_event.is_recuring,
        new_event.owned_by,
        new_event.recurring_by,
    )?;
    let mut option_reminder = None;
    for (i, s) in input.iter().enumerate() {
        match s.as_str() {
            "reminder" | "reminder:" | "--reminder" | "-r" => {
                option_reminder = input.get(i + 1).cloned();
                break;
            }
            _ => {}
        }
    }
    if let Some(s) = option_reminder {
        let mut desc_append = "".to_string();
        let mut r: Result<Reminder, Error> = Err(Error::from(""));
        let tr = parse_reminder(conn, vec![created_event.title.clone(), s.to_string()]);
        if let Err(err) = tr {
            desc_append.push_str(
                format!(
                    "\nEvent's reminder could not be parsed with err:
{:?}\n The event was still created successfully",
                    err
                )
                .as_str(),
            )
        } else {
            r = databaser::create_reminder(
                conn,
                tr.as_ref().unwrap().time_before,
                tr.as_ref().unwrap().event_id,
            )
        }
        if r.is_err() {
            desc_append.push_str("\nEvent's reminder could not be created")
        } else {
            let rstr = format!("\n\nevent created with reminder: {:?}", r);
            desc_append.push_str(rstr.as_str())
        }
        match created_event.description {
            Some(s) => created_event.description = Some(s + desc_append.clone().as_str()),
            None => created_event.description = Some(desc_append.to_string()),
        }
    }

    Ok(created_event.to_calendar())
}
struct TempNewEvent {
    title: String,
    description: String,
    timedate: NaiveDateTime,
    is_recuring: bool,
    owned_by: i64,
    recurring_by: Option<i16>,
}
fn parse_event_args(input: Vec<String>, ctx: Context<'_>) -> Result<TempNewEvent, Error> {
    let mut description_in = None;
    let title = input.get(0).map_or("", |s| s.as_str()).to_string();
    for (i, s) in input.iter().enumerate() {
        match s.as_str() {
            "description:" | "--description" | "-d" | "description" => {
                description_in = input.get(i + 1).cloned();
                break;
            }
            _ => {}
        }
    }
    let description = match description_in {
        Some(d) => d,
        None => "".to_string(),
    };
    let timedate = match input.last() {
        Some(s) => parse_timedate(s.as_str())?,
        None => return Err(Error::from("need at least 1 arg")),
    };
    let is_recuring = input
        .iter()
        .any(|s| s.to_lowercase() == "recurring" || s.to_lowercase() == "is_recurring");
    let owned_by = i64::from(ctx.author().id);
    let recurring_by = match is_recuring {
        true => check_recurrance(input),
        false => None,
    };

    let output = TempNewEvent {
        title,
        description,
        timedate,
        is_recuring,
        owned_by,
        recurring_by,
    };

    Ok(output)
}
fn check_recurrance(input: Vec<String>) -> Option<i16> {
    let mut recurring_in: Option<String> = None;
    for (i, s) in input.iter().enumerate() {
        match s.as_str() {
            "recuring" | "is_recuring" => {
                recurring_in = input.get(i + 1).cloned();
                break;
            }
            _ => {}
        }
    }
    match recurring_in {
        Some(r) => {
            if let Ok(i) = r.parse::<i16>() {
                return Some(i);
            }
            match r.as_str() {
                "daily" | "day" | "dayly" => Some(0),
                "weekly" | "week" => Some(1),
                "month" | "monthly" => Some(2),
                "yearly" | "year" => Some(3),
                _ => None,
            }
        }
        None => None,
    }
}

fn add_birthday(conn: &mut PgConnection, input: Vec<String>) -> Result<ToddEvent, Error> {
    let new_event = parse_birthday_args(conn, input.clone())?;
    let created_event = databaser::create_event(
        conn,
        &new_event.title,
        &new_event.description,
        new_event.timedate,
        new_event.is_recuring,
        new_event.owned_by,
        new_event.recurring_by,
    )?;
    Ok(created_event)
}
fn parse_birthday_args(conn: &mut PgConnection, input: Vec<String>) -> Result<TempNewEvent, Error> {
    if input.is_empty() {
        return Err(Error::from("Error: not enough args"));
    }
    if input[0].is_empty() {
        return Err(Error::from("Error: not enough args"));
    }
    let parsed_member = todd_commands::parse_member_or_return_lowercase(&input[0]);
    let owner_member = databaser::get_member(conn, &parsed_member)?;
    let output = TempNewEvent {
        title: "Birthday".to_string(),
        description: format!(
            "# Its {}'s birthday today!\nHappy birthday <@{}>",
            owner_member.primary_name, owner_member.id
        ),
        timedate: match input.last() {
            Some(s) => parse_timedate(s.as_str())?,
            None => return Err(Error::from("need at least 1 arg")),
        },
        is_recuring: true,
        owned_by: owner_member.id as i64,
        recurring_by: Some(3), // yearly
    };
    Ok(output)
}

fn create_yearly_reminder(conn: &mut PgConnection, event: ToddEvent) -> Result<Reminder, Error> {
    let current_datetime = Local::now().naive_local();
    let diff: i32 = current_datetime.month() as i32 - event.timedate.month() as i32;
    let new_time = if diff.is_positive()
        || (diff == 0 && event.timedate.day() <= current_datetime.day())
    // TODO:
    // Reminders are not being created at the right year...
    {
        event.timedate.with_year(current_datetime.year() + 1)
    } else {
        event.timedate.with_year(current_datetime.year())
    }
    .ok_or_else(|| Error::from("New datetime out of scope"))?;

    // Expected End:
    let new_reminder = databaser::create_reminder(conn, new_time, event.id)?;
    Ok(new_reminder)
}
fn create_monthly_reminder(conn: &mut PgConnection, event: ToddEvent) -> Result<Reminder, Error> {
    let current = Local::now().naive_local();
    let timedate = event.timedate;
    let diff: i64 = { current.month0() as i64 - timedate.month0() as i64 };
    let new_time = if diff == 0 && timedate.day0() >= current.day0() || (diff.is_negative()) {
        timedate.with_month0(current.month0())
    } else {
        timedate.with_month0(current.month0() + 1)
    }
    .and_then(|dt| dt.with_year(current.year()))
    .ok_or_else(|| Error::from("Invalid timedate"))?;

    let new_reminder = databaser::create_reminder(conn, new_time, event.id)?;

    Ok(new_reminder)
}
fn create_weekly_reminder(conn: &mut PgConnection, event: ToddEvent) -> Result<Reminder, Error> {
    let current = Local::now().naive_local();
    let timedate = event.timedate;
    let diff: i64 = {
        current.weekday().num_days_from_sunday() as i64
            - timedate.weekday().num_days_from_sunday() as i64
    };
    let new_time = if diff == 0
        && timedate.num_seconds_from_midnight() > current.num_seconds_from_midnight()
    {
        current
    } else {
        current - chrono::Duration::days(diff + 7)
    }
    .with_hour(timedate.hour())
    .and_then(|dt| dt.with_minute(timedate.minute()))
    .and_then(|dt| dt.with_second(timedate.second()))
    .ok_or_else(|| Error::from("Invalid time components"))?;

    let new_reminder = databaser::create_reminder(conn, new_time, event.id)?;
    Ok(new_reminder)
}
fn create_daily_reminder(conn: &mut PgConnection, event: ToddEvent) -> Result<Reminder, Error> {
    let current = Local::now().naive_local();
    let timedate = event.timedate;
    let diff: i64 = {
        current.num_seconds_from_midnight() as i64 - timedate.num_seconds_from_midnight() as i64
    };
    let new_time = if !diff.is_negative() {
        current - chrono::Duration::seconds(diff)
    } else {
        current - chrono::Duration::seconds(diff + 86400)
    };

    let new_reminder = databaser::create_reminder(conn, new_time, event.id)?;
    Ok(new_reminder)
}

struct TempNewReminder {
    time_before: NaiveDateTime,
    event_id: i32,
}
fn add_reminder(conn: &mut PgConnection, input: Vec<String>) -> Result<Reminder, Error> {
    let new_reminder = parse_reminder(conn, input.clone())?;
    let created_reminder =
        databaser::create_reminder(conn, new_reminder.time_before, new_reminder.event_id)?;
    Ok(created_reminder)
}
fn parse_reminder(conn: &mut PgConnection, input: Vec<String>) -> Result<TempNewReminder, Error> {
    if input.is_empty() {
        return Err(Error::from("Error: need at least 1 arg"));
    }
    let last_input = if let Some(s) = input.last() {
        s.as_str()
    } else {
        return Err(Error::from("need at least 1 arg"));
    };
    let event_title = if input[0] == "for" {
        &input[1]
    } else {
        &input[0]
    };
    let event = databaser::get_event_by_title(conn, event_title)?;
    if event.is_empty() {
        return Err(Error::from("no events found"));
    }
    let output = TempNewReminder {
        time_before: if let Ok(p) = parse_timedate(last_input) {
            p
        } else if let Some(s) = last_input.strip_suffix("before") {
            let strings = if let Some(t) = s.trim().split_once(' ') {
                t
            } else {
                return Err(Error::from("Must specify n timeunits before"));
            };
            let seconds_before = strings.0.parse::<i64>()? * {
                if strings.0.parse::<i64>()?.is_negative() {
                    return Err(Error::from("time must be positive"));
                } else if strings.1 == "seconds" {
                    1
                } else if strings.1 == "minutes" {
                    60
                } else if strings.1 == "hours" {
                    3600
                } else if strings.1 == "days" {
                    3600 * 24
                } else if strings.1 == "weeks" {
                    86400 * 7
                } else if strings.1 == "months" {
                    2592000
                } else if strings.1 == "years" {
                    31449600
                } else {
                    return Err(Error::from("must specify unit of time"));
                }
            };
            let new_timestamp = event[0].timedate.timestamp() - seconds_before;
            if let Some(t) = NaiveDateTime::from_timestamp_opt(new_timestamp, 0) {
                t
            } else {
                return Err(Error::from("Error: invalid timestamp"));
            }
        } else {
            println!("Current inputs: {:?}\n", input);
            return Err(Error::from("Error: wrong format"));
        },
        event_id: event[0].id,
    };
    Ok(output)
}
fn parse_timedate(input: &str) -> Result<NaiveDateTime, Error> {
    let formats_to_try: Vec<&str> = vec!["%D %I:%M %P", "%D %I:%M %p", "%D %R", "%D"];
    for f in formats_to_try {
        if let Ok(d) = NaiveDateTime::parse_from_str(input, f) {
            return Ok(d);
        } else if let Ok(n) = NaiveDate::parse_from_str(input, f) {
            return Ok(n.and_hms_opt(7, 0, 0).unwrap());
        }
    }
    Err(Error::from("Failed to parse input into a datetime"))
}
#[poise::command(prefix_command/*, member_cooldown = 30*/)]
async fn remove(ctx: Context<'_>, event_type: String, event: String) -> Result<(), Error> {
    let mut conn = databaser::establish_connection()?;

    let removed_event: CalendarType = match event_type.to_lowercase().as_str() {
        "event" => remove_event(&mut conn, event)?.to_calendar(),
        "birthday" => remove_birthday(&mut conn, event)?.to_calendar(),
        "reminder" => remove_reminder(&mut conn, event)?.to_calendar(),
        _ => {
            return Err(Error::from(
                "Error parsing type, specify `event` `birthday` or `reminder`",
            ))
        }
    };

    ctx.say(format!(
        "{} has deleted the {}: *{}*",
        ctx.author(),
        event_type,
        removed_event.title()
    ))
    .await?;

    Ok(())
}
fn remove_event(conn: &mut PgConnection, input: String) -> Result<ToddEvent, Error> {
    if input.is_empty() {
        return Err(Error::from("Error: not enough args"));
    }
    let event = if let Ok(e) = databaser::get_event(conn, &input) {
        if e.is_empty() {
            return Err(Error::from(format!("Error: event *{}* not found", &input)));
        } else if e.len() != 1 {
            return Err(Error::from(format!(
                "Error: more than one event found: {:?}
try removing event by it's `id`",
                e
            )));
        } else {
            e[0].clone()
        }
    } else {
        return Err(Error::from(format!("Error: event *{}* not found", &input)));
    };
    databaser::delete_event_by_id(conn, event.id)?;
    Ok(event)
}
fn remove_birthday(conn: &mut PgConnection, input: String) -> Result<ToddEvent, Error> {
    if input.is_empty() {
        return Err(Error::from("Error: not enough args"));
    }
    let poise_member = todd_commands::parse_member_or_return_lowercase(&input);
    let member = databaser::get_member(conn, &poise_member)?;
    let event = databaser::get_birthday(conn, member)?;
    databaser::delete_event_by_id(conn, event.id)?;
    Ok(event)
}
fn remove_reminder(conn: &mut PgConnection, input: String) -> Result<Reminder, Error> {
    if input.is_empty() {
        return Err(Error::from("Error: not enough args"));
    }
    let output = databaser::get_reminder_from_id(
        conn,
        if let Ok(i) = input.parse::<i32>() {
            i
        } else {
            return Err(Error::from(format!("Error: *{}* is not an id", &input)));
        },
    )?;
    databaser::delete_reminder_by_id(conn, output.id)?;

    Ok(output)
}
#[poise::command(
    prefix_command,
    member_cooldown = 30,
    subcommands("events", "reminders")
)]
async fn list(ctx: Context<'_>, input: String) -> Result<(), Error> {
    let mut conn = databaser::establish_connection()?;
    let mut body = "".to_string();
    let parent = databaser::get_event(&mut conn, &input)?;
    for e in parent {
        let reminders_vec = databaser::get_reminders_from_event(&mut conn, &e);
        body.push_str(
            format!(
                "\n### Event **{}:**\n```{:?}```{}'s reminders:\n```{:?}```",
                e.title, e, e.title, reminders_vec
            )
            .as_str(),
        )
    }
    ctx.reply(body).await?;
    Ok(())
}
#[poise::command(prefix_command, member_cooldown = 30)]
async fn events(ctx: Context<'_>) -> Result<(), Error> {
    let mut conn = databaser::establish_connection()?;
    let mut body = "".to_string();
    let v = databaser::get_all_events(&mut conn)?;
    for e in v {
        let owner = databaser::get_member(&mut conn, e.owned_by.to_string().as_str());
        let mut check: Result<(), ()> = Err(());
        if owner.is_ok() {
            check = Ok(())
        }

        let recurring = if e.is_recuring {
            match e.recurring_by {
                Some(0) => "daily",
                Some(1) => "weekly",
                Some(2) => "monthly",
                Some(3) => "yearly",
                None => "marked as recurring but doesnt have a timeframe",
                _ => "marked as recurring but has an invalid timeframe",
            }
        } else {
            "no"
        };
        let owner = match check {
            Ok(_) => owner.unwrap().primary_name,
            _ => "unknown".to_string(),
        };
        body.push_str(
            format!(
                "\n## {}\n- id: {}\n- owned by: {}\n- is recurring?: {}",
                e.title, e.id, owner, recurring
            )
            .as_str(),
        )
    }
    ctx.reply(body).await?;
    Ok(())
}
#[poise::command(prefix_command, member_cooldown = 30)]
async fn reminders(ctx: Context<'_>, input: Option<String>) -> Result<(), Error> {
    let mut conn = databaser::establish_connection()?;
    let mut body = "".to_string();
    let mut v = vec![];
    if let Some(s) = input {
        if let Ok(events) = databaser::get_event(&mut conn, &s) {
            for e in events {
                if let Ok(r) = databaser::get_reminders_from_event(&mut conn, &e) {
                    for reminder in r {
                        v.push(reminder)
                    }
                }
            }
        }
    } else {
        let reminders = databaser::get_all_reminders(&mut conn)?;
        for r in reminders {
            v.push(r)
        }
    }
    for r in v {
        let parent: CalendarType = {
            let p = r.parent(&mut conn);
            if let Some(s) = p {
                s.to_calendar()
            } else {
                r.clone().to_calendar()
            }
        };
        body.push_str(
            format!(
                "\n### {}\n- {:?}\n- id: {}",
                parent.title(),
                r.time_before,
                r.id
            )
            .as_str(),
        )
    }

    ctx.reply(body).await?;
    Ok(())
}

#[cfg(test)]
mod calendar_tests {
    use super::*;

    #[test]
    fn test_parse_timedate_valid_formats() {
        let valid_inputs = vec![
            "06/08/23 12:34 PM",
            "06/08/23 12:34 pm",
            "06/08/23 12:34",
            "06/08/23",
        ];

        for input in valid_inputs {
            assert!(parse_timedate(input).is_ok(), "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_parse_timedate_invalid_format() {
        let invalid_input = "invalid_date_format";

        assert!(
            parse_timedate(invalid_input).is_err(),
            "Failed for invalid input"
        );
    }
    #[test]
    fn test_parse_reminder() -> Result<(), Error> {
        let valid_inputs = vec![
            vec!["Sample Event".to_string(), "15 minutes before".to_string()],
            vec!["Sample Event".to_string(), "12/11/56 12:00 am".to_string()],
            vec![
                "for".to_string(),
                "Sample Event".to_string(),
                "12/11/56 12:00 am".to_string(),
            ],
            vec!["Sample Event".to_string(), "12/11/56".to_string()],
        ];
        let invalid_inputs = vec![
            vec![],
            vec!["".to_string(), "".to_string()],
            vec!["Fake Event".to_string(), "15 minutes before".to_string()],
            vec!["Sample Event".to_string(), "15 before".to_string()],
            vec!["Sample Event".to_string(), "15 foobar before".to_string()],
            vec!["Sample Event".to_string(), "15 foo bar before".to_string()],
            vec!["Sample Event".to_string(), "-15 minutes before".to_string()],
        ];
        let mut conn = databaser::establish_connection()?;
        let member = databaser::get_member(&mut conn, "paddy")?;
        let sample_event = databaser::create_event(
            &mut conn,
            "Sample Event",
            "Foo Bar",
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(2056, 12, 11).unwrap(),
                NaiveTime::from_hms_opt(13, 0, 0).unwrap(),
            ),
            false,
            member.id,
        )?;

        for input in valid_inputs {
            if let Err(e) = parse_reminder(&mut conn, input.clone()) {
                databaser::delete_event_by_id(&mut conn, sample_event.id)?;
                return Err(e);
            }
        }
        for input in invalid_inputs {
            if let Err(e) = parse_reminder(&mut conn, input.clone()) {
            } else {
                databaser::delete_event_by_id(&mut conn, sample_event.id)?;
                return Err(Error::from(format!("Failed for input: {:?}", input)));
            }
        }

        databaser::delete_event_by_id(&mut conn, sample_event.id)?;
        Ok(())
    }
    #[test]
    fn test_add_birthday() -> Result<(), Error> {
        let mut conn = databaser::establish_connection()?;
        let sample_member = databaser::create_member(&mut conn, 123, "sample", false)?;
        println!("sample member: {:?}", sample_member);
        let valid_inputs = vec![
            vec!["Sample".to_string(), "12/01/99".to_string()],
            vec!["123".to_string(), "12/01/99".to_string()],
        ];
        let invalid_inputs = vec![
            vec![],
            vec!["".to_string(), "".to_string()],
            vec!["FooBar".to_string(), "12/01/99".to_string()],
            vec!["Sample".to_string(), "".to_string()],
            vec!["Sample".to_string(), "Foo".to_string()],
        ];
        for input in valid_inputs {
            if let Err(e) = parse_birthday_args(&mut conn, input.clone()) {
                println!("Failed to parse valid input: {:?}", input);
                databaser::remove_member(&mut conn, 123)?;
                println!("Returning Err:");
                return Err(e);
            }
        }
        for input in invalid_inputs {
            if let Err(_) = parse_birthday_args(&mut conn, input.clone()) {
            } else {
                println!("Failed to parse invalid input: {:?}", input);
                databaser::remove_member(&mut conn, 123)?;
                println!("Returning Err:");
                return Err(Error::from(format!("Invalid Input failed: {:?}", input)));
            }
        }
        databaser::remove_member(&mut conn, 123)?;
        Ok(())
    }
    // #[test]
    // fn test_yearly_recursive_reminders() -> Result<(), Error> {
    //     let current: NaiveDateTime = {
    //         NaiveDateTime::new(
    //             NaiveDate::from_ymd_opt(2023, 12, 30)
    //                 .ok_or_else(|| Error::from("Invalid Current Date"))?,
    //             NaiveTime::from_hms_opt(19, 35, 42)
    //                 .ok_or_else(|| Error::from("Invalid Current Time"))?
    //         )
    //     }; // NOTE: 12/30/23 19:35:42
    //     let events = vec![];
    //     events.push(current - chrono::Duration::days(1));
    //     events.push(current + chrono::Duration::days(1));
    //     events.push(current - chrono::Duration::weeks(4));
    //     events.push(current + chrono::Duration::weeks(4));
    //     events.push(current - chrono::Duration::weeks(50));
    //     events.push(current + chrono::Duration::weeks(60));

    //     let conn = databaser::establish_connection()?;

    //     for event in events {
    //         let expected_result = match event.month() < current.month()
    //             || (event.month() == current.month() && event.day() <= current.day())
    //         {
    //             true => event.with_year(current.year() + 1),
    //             false => event.with_year(current.year()),
    //         };

    //         assert_eq!(
    //             test_create_yearly_reminder(event, current)?,
    //             expected_result,
    //             "Mismatch for event: {:?}",
    //             event
    //         );
    //     }
    //     Ok(())
    // }

    // fn test_create_yearly_reminder(
    //     conn: &mut PgConnection,
    //     event: NaiveDateTime,
    //     current_datetime: NaiveDateTime,
    // ) -> Result<NaiveDateTime, Error> {
    //     let new_time = if event.month() < current_datetime.month()
    //         || (event.month() == current_datetime.month()
    //             && event.day() <= current_datetime.day())
    //     {
    //         event.with_year(current_datetime.year() + 1)
    //     } else {
    //         event.with_year(current_datetime.year())
    //     }
    //     .ok_or_else(|| Error::from("New datetime out of scope"))?;

    //     // Expected End:
    //     Ok(new_time)
    // }
    // fn create_montly_reminder(
    //     conn: &mut PgConnection,
    //     event: NaiveDateTime,
    //     current: NaivedateTime
    // ) -> Result<NaiveDateTime, Error> {
    //     let diff: i64 = { current.month0() as i64 - event.month0() as i64 };
    //     let new_time = if diff == 0 && event.day0() >= current.day0() || (diff.is_negative()) {
    //         event.with_month0(current.month0())
    //     } else {
    //         event.with_month0(current.month0() + 1)
    //     }
    //     .and_then(|dt| dt.with_year(current.year()))
    //     .ok_or_else(|| Error::from("Invalid event"))?;

    //     Ok(new_time)
    // }
    // fn create_weekly_reminder(
    //     conn: &mut PgConnection,
    //     event: NaiveDateTime,
    //     current: NaiveDateTime,
    // ) -> Result<NaiveDateTime, Error> {
    //     let diff: i64 = {
    //         current.weekday().num_days_from_sunday() as i64
    //             - event.weekday().num_days_from_sunday() as i64
    //     };
    //     let new_time = if diff == 0
    //         && event.num_seconds_from_midnight() > current.num_seconds_from_midnight()
    //     {
    //         current
    //     } else {
    //         current - chrono::Duration::days(diff + 7)
    //     }
    //     .with_hour(event.hour())
    //     .and_then(|dt| dt.with_minute(event.minute()))
    //     .and_then(|dt| dt.with_second(event.second()))
    //     .ok_or_else(|| Error::from("Invalid time components"))?;

    //     Ok(new_time)
    // }
    // fn create_daily_reminder(
    //     conn: &mut PgConnection,
    //     event: NaiveDateTime,
    //     current: NaiveDateTime,
    // ) -> Result<NaiveDateTime, Error> {
    //     let diff: i64 = {
    //         current.num_seconds_from_midnight() as i64
    //             - event.num_seconds_from_midnight() as i64
    //     };
    //     let new_time = if !diff.is_negative() {
    //         current - chrono::Duration::seconds(diff)
    //     } else {
    //         current - chrono::Duration::seconds(diff + 86400)
    //     };

    //     Ok(new_time)
    // }

    // fn test_remove_event() {
    //     let valid_inputs: Vec<String> = vec![
    //         "Sample Event",
    //         "123123"
    //     ];
    //     let invalid_input: Vec<String> = vec![
    //         "Foo Bar Event",
    //         "44444444",
    //         "",
    //         "Multi Event"
    //     ];
    // }
}
// TODO: fn check_upcoming_events
// TODO: have reminders get deleted after they are sent
// also have events that are not recuring delete after they pass

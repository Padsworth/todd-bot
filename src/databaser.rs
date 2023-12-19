// databaser.rs

use crate::models::{
    NewEvent, NewMember, NewNickname, NewQuote, NewReminder, Nickname, Quote, Reminder,
    SchlonghouseMember, ToddEvent,
};
use crate::Error;
use chrono::prelude::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use std::env::var;

// My perception is that this code could really be cleaned up
// via fixing the queries. My understanding of the diesel api
// is pretty weak. Hopefully could create one big query:
// `get_member_from_id_name_or_nickname` that would prolly
// just be called `get_member`

pub fn establish_connection() -> Result<PgConnection, Error> {
    dotenv().ok();

    let database_url = var("DATABASE_URL")
        .map_err(|e| Error::from(format!("Failed to load DATABASE_URL: {}", e)))?;
    PgConnection::establish(&database_url)
        .map_err(|e| Error::from(format!("Error connecting to {}: {}", database_url, e)))
}

pub fn create_member(
    conn: &mut PgConnection,
    member_id: i64,
    member_primary_name: &str,
    member_is_member: bool,
) -> Result<SchlonghouseMember, Error> {
    use crate::schema::members;

    let new_member = NewMember {
        id: member_id,
        primary_name: member_primary_name,
        is_member: member_is_member,
    };

    let output = diesel::insert_into(members::table)
        .values(&new_member)
        .get_result(conn)?;

    Ok(output)
}

pub fn create_quote(conn: &mut PgConnection, quoted: &str, quote: &str) -> Result<Quote, Error> {
    use crate::schema::quotes;

    let new_quote = NewQuote { quoted, quote };

    let output = diesel::insert_into(quotes::table)
        .values(&new_quote)
        .get_result(conn)?;

    Ok(output)
}

pub fn create_nickname(
    conn: &mut PgConnection,
    member: &SchlonghouseMember,
    new_nickname: &str,
) -> Result<Nickname, Error> {
    use crate::schema::nicknames;

    let new_nickname = NewNickname {
        nickname: new_nickname,
        primary_name: member.id,
    };

    let output = diesel::insert_into(nicknames::table)
        .values(&new_nickname)
        .get_result(conn)?;

    Ok(output)
}

pub fn get_member(conn: &mut PgConnection, member_id: &str) -> Result<SchlonghouseMember, Error> {
    let output = if let Ok(parsed_id) = member_id.parse::<i64>() {
        get_member_from_id(conn, parsed_id)?
    } else {
        get_member_from_name(conn, member_id)?
    };
    Ok(output)
}

fn get_member_from_id(
    conn: &mut PgConnection,
    member_id: i64,
) -> Result<SchlonghouseMember, Error> {
    use crate::schema::members::dsl::*;
    let output = members.find(member_id).first(conn)?;
    Ok(output)
}

fn get_member_from_primary_name(
    conn: &mut PgConnection,
    name: &str,
) -> Result<SchlonghouseMember, Error> {
    use crate::schema::members::dsl::*;
    let output = members.filter(primary_name.eq(name)).first(conn)?;
    Ok(output)
}

fn get_member_from_nickname(
    conn: &mut PgConnection,
    nickname_to_check: &str,
) -> Result<SchlonghouseMember, Error> {
    use crate::schema::members::dsl::*;
    use crate::schema::nicknames::dsl::*;
    let result = nicknames
        .inner_join(members)
        .filter(nickname.eq(nickname_to_check))
        .select((
            crate::schema::members::id,
            crate::schema::members::primary_name,
            crate::schema::members::is_member,
        ))
        .first(conn)?;
    Ok(result)
}

pub fn get_member_from_name(
    conn: &mut PgConnection,
    name_input: &str,
) -> Result<SchlonghouseMember, Error> {
    let output = if let Ok(member) = get_member_from_primary_name(conn, name_input) {
        member
    } else {
        get_member_from_nickname(conn, name_input)?
    };
    Ok(output)
}

pub fn get_all_members_quotes(conn: &mut PgConnection, owner: &str) -> Result<Vec<Quote>, Error> {
    use crate::schema::quotes;

    let output = quotes::table
        .filter(quotes::quoted.eq(owner))
        .load::<Quote>(conn)?;

    Ok(output)
}

pub fn get_random_quote_from_quotes(quote_vector: Vec<Quote>) -> Result<String, Error> {
    use rand::Rng;

    if quote_vector.is_empty() {
        return Err(Error::from("quotes list is empty"));
    }
    let random_index = rand::thread_rng().gen_range(0..quote_vector.len());
    let output = quote_vector
        .get(random_index)
        .ok_or(Error::from("Error while indexing a random quote"))?;

    Ok(output.quote.clone())
}

pub fn remove_member(conn: &mut PgConnection, member_id: i64) -> Result<(), Error> {
    use crate::schema::members;
    diesel::delete(members::table.filter(members::id.eq(member_id))).execute(conn)?;
    Ok(())
}
pub fn create_event(
    conn: &mut PgConnection,
    new_title: &str,
    new_description: &str,
    when: NaiveDateTime,
    recuring: bool,
    owned_by_member_id: i64,
    recurring_by_num: Option<i16>,
) -> Result<ToddEvent, Error> {
    use crate::schema::events;
    let new_event = NewEvent {
        title: new_title,
        description: new_description,
        timedate: when,
        is_recuring: recuring,
        owned_by: owned_by_member_id,
        recurring_by: recurring_by_num,
    };
    let output = diesel::insert_into(events::table)
        .values(&new_event)
        .get_result(conn)?;
    Ok(output)
}
pub fn delete_event_by_id(conn: &mut PgConnection, event_id_to_delete: i32) -> Result<(), Error> {
    use crate::schema::events;
    diesel::delete(events::table.filter(events::id.eq(event_id_to_delete))).execute(conn)?;
    Ok(())
}
pub fn get_event(conn: &mut PgConnection, event: &str) -> Result<Vec<ToddEvent>, Error> {
    if let Ok(p) = event.parse::<i32>() {
        let output = vec![get_event_by_id(conn, p)?];
        Ok(output)
    } else {
        get_event_by_title(conn, event)
    }
}
pub fn get_event_by_title(
    conn: &mut PgConnection,
    event_title: &str,
) -> Result<Vec<ToddEvent>, Error> {
    use crate::schema::events::dsl::*;
    let output = events
        .filter(title.eq(event_title))
        .load::<ToddEvent>(conn)?;
    Ok(output)
}
pub fn get_event_by_id(conn: &mut PgConnection, event_id: i32) -> Result<ToddEvent, Error> {
    use crate::schema::events::dsl::*;
    let output = events.filter(id.eq(event_id)).first(conn)?;
    Ok(output)
}
pub fn get_birthday(
    conn: &mut PgConnection,
    member: SchlonghouseMember,
) -> Result<ToddEvent, Error> {
    use crate::schema::events::dsl::*;
    let output = events
        .filter(title.eq("Birthday".to_string()))
        .filter(owned_by.eq(member.id))
        .first(conn)?;
    Ok(output)
}
pub fn create_reminder(
    conn: &mut PgConnection,
    new_time_before: NaiveDateTime,
    owned_by_event_id: i32,
) -> Result<Reminder, Error> {
    use crate::schema::reminders;
    let new_reminder = NewReminder {
        time_before: &new_time_before,
        event_id: owned_by_event_id,
    };
    let output = diesel::insert_into(reminders::table)
        .values(&new_reminder)
        .get_result(conn)?;
    Ok(output)
}
pub fn delete_reminder_by_id(
    conn: &mut PgConnection,
    reminder_id_to_delete: i32,
) -> Result<(), Error> {
    use crate::schema::reminders;
    diesel::delete(reminders::table.filter(reminders::id.eq(reminder_id_to_delete)))
        .execute(conn)?;
    Ok(())
}
pub fn get_reminders_from_event(
    conn: &mut PgConnection,
    todd_event: &ToddEvent,
) -> Result<Vec<Reminder>, Error> {
    use crate::schema::reminders;
    let event_reminders = reminders::table
        .filter(reminders::event_id.eq(todd_event.id))
        .load::<Reminder>(conn)?;
    Ok(event_reminders)
}
pub fn get_reminder_from_id(conn: &mut PgConnection, input_id: i32) -> Result<Reminder, Error> {
    use crate::schema::reminders::dsl::*;
    let output = reminders.filter(id.eq(input_id)).first(conn)?;
    Ok(output)
}
pub fn get_all_events(conn: &mut PgConnection) -> Result<Vec<ToddEvent>, Error> {
    use crate::schema::events::dsl::*;
    let output = events.load::<ToddEvent>(conn)?;
    Ok(output)
}
pub fn get_all_reminders(conn: &mut PgConnection) -> Result<Vec<Reminder>, Error> {
    use crate::schema::reminders::dsl::*;
    let output = reminders.load::<Reminder>(conn)?;
    Ok(output)
}

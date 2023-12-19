// models.rs
use crate::schema::{events, members, nicknames, quotes, reminders};
use chrono::prelude::*;
use diesel::prelude::*;
use std::fmt;

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SchlonghouseMember {
    pub id: i64,
    pub primary_name: String,
    pub is_member: bool,
}

#[derive(Queryable, Identifiable, Selectable, Debug, Associations, PartialEq)]
#[diesel(belongs_to(SchlonghouseMember, foreign_key = primary_name))]
#[diesel(table_name = nicknames)]
pub struct Nickname {
    pub id: i32,
    pub nickname: String,
    pub primary_name: i64,
}
#[derive(Debug, Insertable, Associations)]
#[diesel(belongs_to(SchlonghouseMember, foreign_key = primary_name))]
#[diesel(table_name = nicknames)]
pub struct NewNickname<'a> {
    pub nickname: &'a str,
    pub primary_name: i64,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = members)]
pub struct NewMember<'a> {
    pub id: i64,
    pub primary_name: &'a str,
    pub is_member: bool,
}
#[derive(Queryable, Identifiable, Selectable, Debug, PartialEq)]
#[diesel(belongs_to(SchlonghouseMember))]
#[diesel(table_name = quotes)]
pub struct Quote {
    pub id: i32,
    pub quoted: String,
    pub quote: String,
}

#[derive(Debug, Insertable)]
#[diesel(belongs_to(SchlonghouseMember))]
#[diesel(table_name = quotes)]
pub struct NewQuote<'a> {
    pub quoted: &'a str,
    pub quote: &'a str,
}
#[derive(Clone, Queryable, Identifiable, Selectable, Debug, Associations, PartialEq)]
#[diesel(belongs_to(SchlonghouseMember, foreign_key = owned_by))]
#[diesel(table_name = events)]
pub struct ToddEvent {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub timedate: NaiveDateTime,
    pub is_recuring: bool,
    pub owned_by: i64,
    pub recurring_by: Option<i16>,
}
#[derive(Debug, Insertable, Associations)]
#[diesel(belongs_to(SchlonghouseMember, foreign_key = owned_by))]
#[diesel(table_name = events)]
pub struct NewEvent<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub timedate: NaiveDateTime,
    pub is_recuring: bool,
    pub owned_by: i64,
    pub recurring_by: Option<i16>,
}
#[derive(Clone, Queryable, Identifiable, Selectable, Debug, Associations, PartialEq)]
#[diesel(belongs_to(ToddEvent, foreign_key = event_id))]
#[diesel(table_name = reminders)]
pub struct Reminder {
    pub id: i32,
    pub time_before: NaiveDateTime,
    pub event_id: i32,
}
#[derive(Debug, Insertable, Associations)]
#[diesel(belongs_to(ToddEvent, foreign_key = event_id))]
#[diesel(table_name = reminders)]
pub struct NewReminder<'a> {
    pub time_before: &'a NaiveDateTime,
    pub event_id: i32,
}

#[derive(Debug)]
pub struct BirthdayEvent {
    pub title: String,
    pub description: String,
    pub date: NaiveDateTime,
    pub is_recuring: bool,
    pub member_id: i64,
}
#[derive(Debug, Clone)]
pub enum CalendarType {
    Tevent(ToddEvent),
    Teminder(Reminder),
}
#[derive(Debug)]
pub enum Calendar {
    Event,
    Birthday,
    Reminder,
}
pub struct CalendarParseError;
impl fmt::Display for CalendarParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error parsing Calendar")
    }
}
impl fmt::Debug for CalendarParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Err at {{ {} line {} }}, \nnot event reminder or birthday",
            file!(),
            line!()
        )
    }
}

impl std::str::FromStr for Calendar {
    type Err = CalendarParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("event") {
            Ok(Calendar::Event)
        } else if s.contains("birthday") {
            Ok(Calendar::Birthday)
        } else if s.contains("reminder") {
            Ok(Calendar::Reminder)
        } else {
            Err(CalendarParseError)
        }
    }
}
pub trait ToCalendar {
    fn to_calendar(self) -> CalendarType;
}
impl ToCalendar for ToddEvent {
    fn to_calendar(self) -> CalendarType {
        CalendarType::Tevent(self)
    }
}
impl ToCalendar for Reminder {
    fn to_calendar(self) -> CalendarType {
        CalendarType::Teminder(self)
    }
}
impl CalendarType {
    pub fn title(&self) -> String {
        use crate::databaser;
        match self {
            CalendarType::Tevent(t) => t.title.clone(),
            CalendarType::Teminder(r) => {
                let mut conn = if let Ok(p) = databaser::establish_connection() {
                    p
                } else {
                    return "Reminder".to_string();
                };
                if let Ok(e) = databaser::get_event_by_id(&mut conn, r.event_id) {
                    format!("{} Reminder", e.title)
                } else {
                    "Reminder".to_string()
                }
            }
        }
    }
    pub fn description(&self) -> Option<String> {
        use crate::databaser;
        match self {
            CalendarType::Tevent(t) => t.description.clone(),
            CalendarType::Teminder(r) => {
                let mut conn = match databaser::establish_connection() {
                    Ok(c) => c,
                    Err(_) => return None,
                };
                let parent = match databaser::get_event_by_id(&mut conn, r.event_id) {
                    Ok(p) => p,
                    Err(_) => return None,
                };
                parent.description
            }
        }
    }
    pub fn when(&self) -> NaiveDateTime {
        match self {
            CalendarType::Tevent(t) => t.timedate,
            CalendarType::Teminder(r) => r.time_before,
        }
    }
    pub fn is_recuring(&self) -> bool {
        match self {
            CalendarType::Tevent(t) => t.is_recuring,
            CalendarType::Teminder(_) => false,
        }
    }
}
impl Reminder {
    pub fn parent(&self, conn: &mut PgConnection) -> Option<ToddEvent> {
        use crate::databaser;
        if let Ok(p) = databaser::get_event_by_id(conn, self.event_id) {
            return Some(p);
        }
        None
    }
}

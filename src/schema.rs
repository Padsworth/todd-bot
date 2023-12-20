// @generated automatically by Diesel CLI.

diesel::table! {
    events (id) {
        id -> Int4,
        title -> Varchar,
        description -> Nullable<Varchar>,
        timedate -> Timestamp,
        is_recuring -> Bool,
        owned_by -> Int8,
        recurring_by -> Nullable<Int2>,
    }
}

diesel::table! {
    members (id) {
        id -> Int8,
        primary_name -> Varchar,
        is_member -> Bool,
    }
}

diesel::table! {
    nicknames (id) {
        id -> Int4,
        nickname -> Varchar,
        primary_name -> Int8,
    }
}

diesel::table! {
    quotes (id) {
        id -> Int4,
        quoted -> Varchar,
        quote -> Text,
    }
}

diesel::table! {
    reminders (id) {
        id -> Int4,
        time_before -> Timestamp,
        event_id -> Int4,
    }
}

diesel::joinable!(events -> members (owned_by));
diesel::joinable!(nicknames -> members (primary_name));
diesel::joinable!(reminders -> events (event_id));

diesel::allow_tables_to_appear_in_same_query!(events, members, nicknames, quotes, reminders,);

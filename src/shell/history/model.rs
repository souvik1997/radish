use super::diesel::prelude::*;
use super::chrono::naive::NaiveDateTime;
use super::schema::*;

#[derive(Queryable)]
pub struct Entry {
    pub timestamp: NaiveDateTime,
    pub command: String,
}

#[derive(Insertable)]
#[table_name = "history"]
pub struct NewHistoryEntry<'a> {
    pub timestamp: NaiveDateTime,
    pub command: &'a str,
}

use std::cell::Cell;

extern crate diesel;
use self::diesel::prelude::*;
extern crate chrono;

mod schema;
mod model;
pub use self::model::Entry;
use self::model::NewHistoryEntry;
use self::schema::history::dsl::*;
use self::schema::history::columns::timestamp;
use self::schema::history;

pub struct History {
    connection: SqliteConnection,
    entries: Option<Vec<Entry>>,
}

impl History {
    pub fn new(database_url: &str) -> Result<History, ConnectionError> {
        let connection = SqliteConnection::establish(database_url);
        match connection {
            Ok(con) => {
                // Sqlite stores timestamp as text
                diesel::sql_query("CREATE TABLE IF NOT EXISTS history (
                                   timestamp text PRIMARY_KEY,
                                   command text
                                  );").execute(&con).map_err(|e| {
                                      use std::error::Error;
                                      ConnectionError::BadConnection(e.description().to_owned())
                                  })?;
                let entries = History::load_entries(&con);
                Ok(History {
                    connection: con,
                    entries: entries,
                })
            },
            Err(e) => {
                Err(e)
            }
        }
    }

    pub fn len(&self) -> usize {
        match self.entries {
            Some(ref s) => s.len(),
            None => 0
        }
    }

    pub fn entries<'a>(&'a self) -> &'a Option<Vec<Entry>> {
        &self.entries
    }

    fn load_entries(connection: &SqliteConnection) -> Option<Vec<Entry>> {
        match history.order(timestamp.asc()).load(connection) {
            Ok(entries) => Some(entries),
            Err(_) => None,
        }
    }

    pub fn add_command(&mut self, cmd: &str) -> Result<(), ()> {
        match diesel::insert_into(history::table).values(&NewHistoryEntry {
            timestamp: chrono::offset::Utc::now().naive_utc(),
            command: cmd,
        }).execute(&self.connection) {
            Ok(_) => {
                self.entries = History::load_entries(&self.connection);
                Ok(())
            },
            Err(e) => {
                eprintln!("error: {:}", e);
                Err(())
            }
        }
    }
}

use super::conn;
use failure::err_msg;
use failure::Error;
use rusqlite::Row;
use rusqlite::NO_PARAMS;

pub mod message;
pub mod room;

pub use self::message::Message;
pub use self::message::MessageModel;
pub use self::room::Room;

pub trait Model: Sized {
    fn store(&self) -> Result<(), Error>;
    fn get_id(&self) -> &str;
    fn map_row(row: &Row) -> Self;

    fn create_sql() -> String;
    fn table_name() -> &'static str;
    fn fields() -> Vec<&'static str>;

    fn get(id: &str) -> Result<Self, Error> {
        let fields = Self::fields().join(",");
        let query = format!("SELECT {} FROM {} WHERE id = ?", fields, Self::table_name());
        conn(
            move |c| {
                let mut stmt = c.prepare(&query)?;
                let mut iter = stmt.query_map(&[id], Self::map_row)?;

                iter.next()
                    .ok_or(err_msg("Object not found"))?
                    .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn all() -> Result<Vec<Self>, Error> {
        let fields = Self::fields().join(",");
        let query = format!("SELECT {} FROM {}", fields, Self::table_name());
        conn(
            move |c| {
                let mut stmt = c.prepare(&query)?;
                let iter = stmt.query_map(NO_PARAMS, Self::map_row)?;

                let array = iter
                    .filter(|r| r.is_ok())
                    .map(|r| r.unwrap())
                    .collect::<Vec<Self>>();
                Ok(array)
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn delete(&self) -> Result<usize, Error> {
        let query = format!("DELETE from {} WHERE id = ?", Self::table_name());

        conn(
            move |c| {
                c.execute(&query, &[self.get_id()])
                    .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn create_table() -> Result<usize, Error> {
        conn(
            move |c| {
                c.execute(&Self::create_sql(), NO_PARAMS)
                    .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }
}

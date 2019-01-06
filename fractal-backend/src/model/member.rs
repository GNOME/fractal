pub use api::types::Member;
use failure::err_msg;
use failure::Error;

use rusqlite::types::ToSql;
use rusqlite::Row;
use rusqlite::NO_PARAMS;

use super::conn;
use super::Model;

impl Model for Member {
    fn table_name() -> &'static str {
        "member"
    }

    fn get_id(&self) -> &str {
        &self.uid
    }

    fn fields() -> Vec<&'static str> {
        vec!["id", "alias", "avatar"]
    }

    fn create_sql() -> String {
        format!(
            "
        CREATE TABLE if not exists {table} (
            id TEXT PRIMARY KEY,
            alias TEXT,
            avatar TEXT
        )
        ",
            table = Self::table_name()
        )
    }

    fn store(&self) -> Result<(), Error> {
        let fields = Self::fields().join(",");
        let questions = Self::fields()
            .iter()
            .map(|_| "?")
            .collect::<Vec<&str>>()
            .join(",");
        let query = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            Self::table_name(),
            fields,
            questions
        );

        conn(
            move |c| {
                c.execute(&query, &[&self.uid, &self.alias as &ToSql, &self.avatar])
                    .map(|_| ())
                    .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn map_row(row: &Row) -> Self {
        Self {
            uid: row.get(0),
            alias: row.get(1),
            avatar: row.get(2),
        }
    }
}

pub trait MemberModel: Sized {
    fn store_relation(&self, room: &str) -> Result<(), Error>;
    fn delete_relation(&self, room: &str) -> Result<usize, Error>;
    fn update_relation(&self, room: &str) -> Result<(), Error>;
    fn get_range(room: &str, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<Self>, Error>;
    fn delete_relations(room: &str) -> Result<usize, Error>;
    fn create_relation_table() -> Result<(), Error>;
}

impl MemberModel for Member {
    fn create_relation_table() -> Result<(), Error> {
        let query = format!(
            "
        CREATE TABLE if not exists {table}_room (
            uid TEXT NOT NULL,
            room TEXT NOT NULL,

            FOREIGN KEY(room) REFERENCES room(id),
            FOREIGN KEY(uid) REFERENCES {table}(id)
        )
        ",
            table = Self::table_name()
        );

        conn(
            move |c| {
                c.execute(&query, NO_PARAMS)
                    .map(|_| ())
                    .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn store_relation(&self, room: &str) -> Result<(), Error> {
        let query = format!(
            "INSERT INTO {table}_room (uid, room) VALUES (?, ?)",
            table = Self::table_name(),
        );

        conn(
            move |c| {
                c.execute(&query, &[&self.uid, room])
                    .map(|_| ())
                    .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn delete_relation(&self, room: &str) -> Result<usize, Error> {
        let query = format!(
            "DELETE from {table}_room WHERE uid = ? and room = ?",
            table = Self::table_name()
        );

        conn(
            move |c| {
                c.execute(&query, &[self.get_id(), room])
                    .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn delete_relations(room: &str) -> Result<usize, Error> {
        let query = format!(
            "DELETE from {table}_room WHERE room = ?",
            table = Self::table_name()
        );

        conn(
            move |c| {
                c.execute(&query, &[room])
                    .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn update_relation(&self, room: &str) -> Result<(), Error> {
        self.delete_relation(room)?;
        self.store_relation(room)
    }

    /// Returns a list of Members from filtering by `room` roomid ordered by
    /// date
    ///
    /// The param `limit` defines the number of members to return, if it's
    /// None, all members will be returned
    ///
    /// The param `offset` is used to ignore that number of members and start
    /// to return from that. if it's None, the return will be done from the end
    /// of the list.
    fn get_range(room: &str, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<Self>, Error> {
        let fields = Self::fields().join(",");
        let mut query = format!(
            "SELECT {fields} FROM {table} INNER JOIN
                {table}_room ON uid=id
                WHERE room = ? ORDER BY uid desc",
            fields = fields,
            table = Self::table_name()
        );

        if let Some(l) = limit {
            query = query + &format!(" LIMIT {}", l);
        }

        if let Some(o) = offset {
            query = query + &format!(" OFFSET {}", o);
        }

        conn(
            move |c| {
                let mut stmt = c.prepare(&query)?;
                let iter = stmt.query_map(&[room], Self::map_row)?;

                let array = iter
                    .filter(|r| r.is_ok())
                    .map(|r| r.unwrap())
                    .collect::<Vec<Self>>();
                Ok(array)
            },
            Err(err_msg("Connection not init")),
        )
    }
}

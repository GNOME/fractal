use std::collections::HashMap;

use super::conn;
use failure::err_msg;
use failure::Error;
use rusqlite::types::ToSql;
use rusqlite::Row;
use rusqlite::NO_PARAMS;

pub use api::types::Room;

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

impl Model for Room {
    fn table_name() -> &'static str {
        "room"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn fields() -> Vec<&'static str> {
        vec![
            "id",
            "avatar",
            "name",
            "topic",
            "alias",
            "guest_can_join",
            "world_readable",
            "n_members",
            "notifications",
            "highlight",
            "fav",
            "left",
            "inv",
            "direct",
            "prev_batch",
        ]
    }

    fn create_sql() -> String {
        //TODO: implements relations for:
        //  members: MemberList,
        //  messages: Vec<Message>,
        //  inv_sender: Option<Member>,
        // TODO: maybe we should store power_level in the Member Table
        //  power_levels: HashMap<String, i32>,
        format!(
            "
        CREATE TABLE if not exists {} (
            id TEXT PRIMARY KEY,
            avatar TEXT,
            name TEXT,
            topic TEXT,
            alias TEXT,
            guest_can_join BOOLEAN NOT NULL,
            world_readable BOOLEAN NOT NULL,
            n_members NUMBER NOT NULL,
            notifications NUMBER NOT NULL,
            highlight NUMBER NOT NULL,
            fav BOOLEAN NOT NULL,
            left BOOLEAN NOT NULL,
            inv BOOLEAN NOT NULL,
            direct BOOLEAN NOT NULL,
            prev_batch TEXT
        )
        ",
            Self::table_name()
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
                c.execute(
                    &query,
                    &[
                        &self.id,
                        &self.avatar as &ToSql,
                        &self.name,
                        &self.topic,
                        &self.alias,
                        &self.guest_can_join,
                        &self.world_readable,
                        &self.n_members,
                        &self.notifications,
                        &self.highlight,
                        &self.fav,
                        &self.left,
                        &self.inv,
                        &self.direct,
                        &self.prev_batch,
                    ],
                )
                .map(|_| ())
                .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn map_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            avatar: row.get(1),
            name: row.get(2),
            topic: row.get(3),
            alias: row.get(4),
            guest_can_join: row.get(5),
            world_readable: row.get(6),
            n_members: row.get(7),
            notifications: row.get(8),
            highlight: row.get(9),
            fav: row.get(10),
            left: row.get(11),
            inv: row.get(12),
            direct: row.get(13),
            prev_batch: row.get(14),

            inv_sender: None,
            power_levels: HashMap::new(),
            messages: vec![],
            members: HashMap::new(),
        }
    }
}

use std::collections::HashMap;

pub use api::types::Message;
use failure::err_msg;
use failure::Error;

use rusqlite::types::ToSql;
use rusqlite::Row;

use chrono::DateTime;
use chrono::Local;

use serde_json;
use serde_json::Value;

use super::conn;
use super::Model;

impl Model for Message {
    fn table_name() -> &'static str {
        "message"
    }

    // TODO: we need a non optional id here
    fn get_id(&self) -> &str {
        match self.id.as_ref() {
            Some(r) => r,
            None => "",
        }
    }

    fn fields() -> Vec<&'static str> {
        vec![
            "sender",
            "mtype",
            "body",
            "date",
            "room",
            "thumb",
            "url",
            "id",
            "formatted_body",
            "format",
            "source",
            "receipt",
            "redacted",
            "in_reply_to",
            "extra_content",
        ]
    }

    fn create_sql() -> String {
        format!(
            "
        CREATE TABLE if not exists {} (
            sender TEXT NOT NULL,
            mtype TEXT NOT NULL,
            body TEXT NOT NULL,
            date TEXT NOT NULL,
            room TEXT NOT NULL,
            thumb TEXT,
            url TEXT,
            id TEXT PRIMARY KEY,
            formatted_body TEXT,
            format TEXT,
            source TEXT,
            receipt TEXT NOT NULL,
            redacted BOOLEAN NOT NULL,
            in_reply_to TEXT,
            extra_content TEXT,

            FOREIGN KEY(room) REFERENCES room(id)
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

        //TODO: maybe we should add a new table for this?
        let receipt_serialized = serde_json::to_string(&self.receipt)?;
        let date_serialized = serde_json::to_string(&self.date)?;
        let extra_content = serde_json::to_string(&self.extra_content)?;

        conn(
            move |c| {
                c.execute(
                    &query,
                    &[
                        &self.sender,
                        &self.mtype,
                        &self.body,
                        &date_serialized,
                        &self.room,
                        &self.thumb as &ToSql,
                        &self.url,
                        &self.id,
                        &self.formatted_body,
                        &self.format,
                        &self.source,
                        &receipt_serialized,
                        &self.redacted,
                        &self.in_reply_to,
                        &extra_content,
                    ],
                )
                .map(|_| ())
                .map_err(|e| err_msg(e.to_string()))
            },
            Err(err_msg("Connection not init")),
        )
    }

    fn map_row(row: &Row) -> Self {
        let dstr: String = row.get(3);
        let rstr: String = row.get(11);
        let ecstr: String = row.get(14);

        let date: DateTime<Local> = serde_json::from_str(&dstr).unwrap();
        let receipt: HashMap<String, i64> = serde_json::from_str(&rstr).unwrap();
        let extra_content: Option<Value> = serde_json::from_str(&ecstr).unwrap();

        Self {
            sender: row.get(0),
            mtype: row.get(1),
            body: row.get(2),
            date: date,
            room: row.get(4),
            thumb: row.get(5),
            url: row.get(6),
            id: row.get(7),
            formatted_body: row.get(8),
            format: row.get(9),
            source: row.get(10),
            receipt: receipt,
            redacted: row.get(12),
            in_reply_to: row.get(13),
            extra_content: extra_content,
        }
    }
}

pub trait MessageModel: Sized {
    fn get_range(room: &str, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<Self>, Error>;
}

impl MessageModel for Message {
    /// Returns a list of Messages from filtering by `room` roomid ordered by
    /// date
    ///
    /// The param `limit` defines the number of messages to return, if it's
    /// None, all messages will be returned
    ///
    /// The param `offset` is used to ignore that number of messages and start
    /// to return from that.  if it's None, the return will be done from the end
    /// of the list.
    fn get_range(room: &str, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<Self>, Error> {
        let fields = Self::fields().join(",");
        let mut query = format!(
            "SELECT {} FROM {} WHERE room = ? ORDER BY date desc",
            fields,
            Self::table_name()
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

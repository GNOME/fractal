use chrono::prelude::*;
use chrono::DateTime;
use chrono::TimeZone;
use ruma_identifiers::{Error as IdError, RoomId, UserId};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryInto;

//FIXME make properties privat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub sender: UserId,
    pub mtype: String,
    pub body: String,
    pub date: DateTime<Local>,
    pub room: RoomId,
    pub thumb: Option<String>,
    pub url: Option<String>,
    pub id: String,
    pub formatted_body: Option<String>,
    pub format: Option<String>,
    pub source: Option<String>,
    pub receipt: HashMap<UserId, i64>, // This `HashMap` associates the user ID with a timestamp
    pub redacted: bool,
    // The event ID of the message this is in reply to.
    pub in_reply_to: Option<String>,
    // This can be used for the client to add more values to the message on sending
    // for example for images attachment the "info" field can be attached as
    // Some(json!({"info": {"h": 296, "w": 296, "mimetype": "image/png", "orientation": 0, "size": 8796}});
    pub extra_content: Option<JsonValue>,
    pub blurhash: Option<String>,
}

impl PartialEq for Message {
    fn eq(&self, other: &Message) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for Message {
    fn partial_cmp(&self, other: &Message) -> Option<Ordering> {
        if self == other {
            Some(Ordering::Equal)
        } else {
            self.date.partial_cmp(&other.date)
        }
    }
}

impl Message {
    pub fn new(room: RoomId, sender: UserId, body: String, mtype: String) -> Self {
        let date = Local::now();
        Message {
            id: get_txn_id(&room, &body, &date.to_string()),
            sender,
            mtype,
            body,
            date,
            room,
            thumb: None,
            url: None,
            formatted_body: None,
            format: None,
            source: None,
            receipt: HashMap::new(),
            redacted: false,
            in_reply_to: None,
            extra_content: None,
            blurhash: None,
        }
    }

    /// List all supported types. By default a message map a m.room.message event, but there's
    /// other events that we want to show in the message history so we map other event types to our
    /// Message struct, like stickers
    pub fn types() -> [&'static str; 2] {
        ["m.room.message", "m.sticker"]
    }

    /// Helper function to use in iterator filter of a matrix.org json response to filter supported
    /// events
    pub fn supported_event(ev: &&JsonValue) -> bool {
        let type_ = ev["type"].as_str().unwrap_or_default();

        for t in Message::types().iter() {
            if t == &type_ {
                return true;
            }
        }

        false
    }

    /// Parses a matrix.org event and return a Message object
    ///
    /// # Arguments
    ///
    /// * `roomid` - The message room id
    /// * `msg` - The message event as Json
    pub fn parse_room_message(room_id: &RoomId, msg: &JsonValue) -> Result<Message, IdError> {
        let sender: UserId = msg["sender"].as_str().unwrap_or_default().try_into()?;

        let timestamp = msg["origin_server_ts"].as_i64().unwrap_or_default() / 1000;
        let server_timestamp: DateTime<Local> = Local.timestamp(timestamp, 0);

        let id = msg["event_id"].as_str().unwrap_or_default();
        let type_ = msg["type"].as_str().unwrap_or_default();

        let redacted = msg["unsigned"].get("redacted_because") != None;

        let mut message = Message {
            sender,
            date: server_timestamp,
            room: room_id.clone(),
            id: id.to_string(),
            mtype: type_.to_string(),
            body: String::new(),
            url: None,
            thumb: None,
            formatted_body: None,
            format: None,
            source: serde_json::to_string_pretty(&msg).ok(),
            receipt: HashMap::new(),
            redacted,
            in_reply_to: None,
            extra_content: None,
            blurhash: None,
        };

        let c = &msg["content"];
        match type_ {
            "m.room.message" => message.parse_m_room_message(c),
            "m.sticker" => message.parse_m_sticker(c),
            _ => {}
        };

        Ok(message)
    }

    fn parse_m_room_message(&mut self, c: &JsonValue) {
        let mtype = c["msgtype"].as_str().map(String::from).unwrap_or_default();
        let body = c["body"].as_str().map(String::from).unwrap_or_default();
        let formatted_body = c["formatted_body"].as_str().map(String::from);
        let format = c["format"].as_str().map(String::from);

        match mtype.as_str() {
            "m.image" | "m.file" | "m.video" | "m.audio" => {
                let url = c["url"].as_str().map(String::from).unwrap_or_default();
                let mut t = c["info"]["thumbnail_url"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_default();
                if t.is_empty() && !url.is_empty() {
                    t = url.clone();
                }
                let blurhash = c.get("blurhash").map(|blurhash| blurhash.to_string());

                self.url = Some(url);
                self.thumb = Some(t);
                self.blurhash = blurhash;
            }
            "m.text" => {
                // Only m.text messages can be replies for backward compatability
                // https://matrix.org/docs/spec/client_server/r0.4.0.html#rich-replies
                self.in_reply_to = c["m.relates_to"]["m.in_reply_to"]["event_id"]
                    .as_str()
                    .map(String::from);
            }
            _ => {}
        };

        self.mtype = mtype;
        self.body = body;
        self.formatted_body = formatted_body;
        self.format = format;
    }

    fn parse_m_sticker(&mut self, c: &JsonValue) {
        let url = c["url"].as_str().map(String::from).unwrap_or_default();
        let mut t = c["info"]["thumbnail_url"]
            .as_str()
            .map(String::from)
            .unwrap_or_default();
        if t.is_empty() && !url.is_empty() {
            t = url.clone();
        }

        self.body = c["body"].as_str().map(String::from).unwrap_or_default();
        self.url = Some(url);
        self.thumb = Some(t);
    }

    /// Create a vec of Message from a json event list
    ///
    /// * `roomid` - The messages room id
    /// * `events` - An iterator to the json events
    pub fn from_json_events_iter<'a, I>(
        room_id: &RoomId,
        events: I,
    ) -> Result<Vec<Message>, IdError>
    where
        I: Iterator<Item = &'a JsonValue>,
    {
        events
            .filter(Message::supported_event)
            .map(|msg| Message::parse_room_message(&room_id, msg))
            .collect()
    }

    pub fn set_receipt(&mut self, receipt: HashMap<UserId, i64>) {
        self.receipt = receipt;
    }
}
/// Generates an unique transaction id for this message
/// The txn_id is generated using the md5sum of a concatenation of the message room id, the
/// message body and the date.

/// https://matrix.org/docs/spec/client_server/r0.3.0.html#put-matrix-client-r0-rooms-roomid-send-eventtype-txnid
pub fn get_txn_id(room: &RoomId, body: &str, date: &str) -> String {
    let msg = format!("{}{}{}", room, body, date);
    let digest = md5::compute(msg.as_bytes());
    format!("{:x}", digest)
}

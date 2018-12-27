use crate::JsonValue;
use chrono::{prelude::*, DateTime, TimeZone};
use md5;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{cmp::Ordering, collections::HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub sender: String,
    pub mtype: String,
    pub body: String,
    pub date: DateTime<Local>,
    pub room: String,
    pub thumb: Option<String>,
    pub url: Option<String>,
    pub id: Option<String>,
    pub formatted_body: Option<String>,
    pub format: Option<String>,
    pub source: Option<String>,
    pub receipt: HashMap<String, i64>, // This `HashMap` associates the user ID with a timestamp
    pub redacted: bool,
    // The event ID of the message this is in reply to.
    pub in_reply_to: Option<String>,
    // This can be used for the client to add more values to the message on sending
    // for example for images attachment the "info" field can be attached as
    // Some(json!({"info": {"h": 296, "w": 296, "mimetype": "image/png", "orientation": 0, "size": 8796}});
    pub extra_content: Option<JsonValue>,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            sender: String::new(),
            mtype: "m.text".to_string(),
            body: "default".to_string(),
            date: Local.ymd(1970, 1, 1).and_hms(0, 0, 0),
            room: String::new(),
            thumb: None,
            url: None,
            id: None,
            formatted_body: None,
            format: None,
            source: None,
            receipt: HashMap::new(),
            redacted: false,
            in_reply_to: None,
            extra_content: None,
        }
    }
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(self_id), Some(other_id)) = (self.id.clone(), other.id.clone()) {
            self_id == other_id
        } else {
            self.sender == other.sender && self.body == other.body
        }
    }
}

impl PartialOrd for Message {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self == other {
            Some(Ordering::Equal)
        } else {
            self.date.partial_cmp(&other.date)
        }
    }
}

impl Message {
    /// Generates an unique transaction id for this message
    /// The txn_id is generated using the md5sum of a concatenation of the message room id, the
    /// message body and the date.

    /// https://matrix.org/docs/spec/client_server/r0.3.0.html#put-matrix-client-r0-rooms-roomid-send-eventtype-txnid
    pub fn get_txn_id(&self) -> String {
        let msg = format!("{}{}{}", self.room, self.body, self.date.to_string());
        let digest = md5::compute(msg.as_bytes());
        format!("{:x}", digest)
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

        Self::types().iter().any(|t| t == &type_)
    }

    /// Parses a matrix.org event and return a Message object
    ///
    /// # Arguments
    ///
    /// * `room_id` - The message room id
    /// * `msg` - The message event as Json
    pub fn parse_room_message(room_id: &str, msg: &JsonValue) -> Self {
        let sender = msg["sender"].as_str().unwrap_or_default();

        let timestamp = msg["origin_server_ts"].as_i64().unwrap_or(0) / 1000;
        let server_timestamp: DateTime<Local> = Local.timestamp(timestamp, 0);

        let id = msg["event_id"].as_str().unwrap_or_default();
        let type_ = msg["type"].as_str().unwrap_or_default();

        let redacted = msg["unsigned"].get("redacted_because") != None;

        let mut message = Self {
            sender: sender.to_string(),
            date: server_timestamp,
            room: room_id.to_string(),
            id: Some(id.to_string()),
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
        };

        let c = &msg["content"];
        match type_ {
            "m.room.message" => Self::parse_m_room_message(&mut message, c),
            "m.sticker" => Self::parse_m_sticker(&mut message, c),
            _ => {}
        };

        message
    }

    fn parse_m_room_message(msg: &mut Self, c: &JsonValue) {
        let mtype = c["msgtype"].as_str().unwrap_or_default();
        let body = c["body"].as_str().unwrap_or_default();
        let formatted_body = c["formatted_body"].as_str().map(Into::into);
        let format = c["format"].as_str().map(Into::into);

        match mtype {
            "m.image" | "m.file" | "m.video" | "m.audio" => {
                let url = c["url"].as_str().unwrap_or_default().to_string();
                let mut t = c["info"]["thumbnail_url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                if t.is_empty() && !url.is_empty() {
                    t = url.clone();
                }

                msg.url = Some(url);
                msg.thumb = Some(t);
            }
            "m.text" => {
                // Only m.text messages can be replies for backward compatability
                // https://matrix.org/docs/spec/client_server/r0.4.0.html#rich-replies
                msg.in_reply_to = c["m.relates_to"]["m.in_reply_to"]["event_id"]
                    .as_str()
                    .map(Into::into);
            }
            _ => {}
        };

        msg.mtype = mtype.to_string();
        msg.body = body.to_string();
        msg.formatted_body = formatted_body;
        msg.format = format;
    }

    fn parse_m_sticker(msg: &mut Self, c: &JsonValue) {
        let body = c["body"].as_str().unwrap_or_default();

        let url = c["url"].as_str().unwrap_or_default();
        let t = c["info"]["thumbnail_url"]
            .as_str()
            .filter(|t| !t.is_empty())
            .unwrap_or(url.clone());

        msg.body = body.to_string();
        msg.url = Some(url.to_string());
        msg.thumb = Some(t.to_string());
    }

    /// Create a vec of Message from a json event list
    ///
    /// * `room_id` - The messages room id
    /// * `events` - An iterator to the json events
    pub fn from_json_events_iter<'a, I>(room_id: &str, events: I) -> Vec<Self>
    where
        I: Iterator<Item = &'a JsonValue>,
    {
        events
            .filter(Self::supported_event)
            .map(|msg| Self::parse_room_message(room_id, msg))
            .collect()
    }

    pub fn set_receipt(&mut self, receipt: HashMap<String, i64>) {
        self.receipt = receipt;
    }
}

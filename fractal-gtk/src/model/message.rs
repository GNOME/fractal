use chrono::prelude::*;
use chrono::DateTime;
use chrono::TimeZone;
use fractal_api::{
    identifiers::{Error as IdError, EventId, RoomId, UserId},
    url::Url,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryInto;
use std::path::PathBuf;

//FIXME make properties private
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub sender: UserId,
    pub mtype: String,
    pub body: String,
    pub date: DateTime<Local>,
    pub room: RoomId,
    pub thumb: Option<Url>,
    pub local_path_thumb: Option<PathBuf>,
    pub url: Option<Url>,
    pub local_path: Option<PathBuf>,
    // FIXME: This should be a required field but it is mandatory
    // to do it this way because because this struct is used both
    // for received messages and messages to send. At the moment
    // of writing this, using two separate data structures for each
    // use case is just too difficult.
    pub id: Option<EventId>,
    pub formatted_body: Option<String>,
    pub format: Option<String>,
    pub source: Option<String>,
    pub receipt: HashMap<UserId, i64>, // This `HashMap` associates the user ID with a timestamp
    pub redacted: bool,
    // The event ID of the message this is in reply to.
    pub in_reply_to: Option<EventId>,
    // The event ID of the message this replaces.
    pub replace: Option<EventId>,
    // This can be used for the client to add more values to the message on sending
    // for example for images attachment the "info" field can be attached as
    // Some(json!({"info": {"h": 296, "w": 296, "mimetype": "image/png", "orientation": 0, "size": 8796}});
    pub extra_content: Option<JsonValue>,
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
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
    pub fn new(
        room: RoomId,
        sender: UserId,
        body: String,
        mtype: String,
        id: Option<EventId>,
    ) -> Self {
        let date = Local::now();
        Message {
            id,
            sender,
            mtype,
            body,
            date,
            room,
            thumb: None,
            local_path_thumb: None,
            url: None,
            local_path: None,
            formatted_body: None,
            format: None,
            source: None,
            receipt: HashMap::new(),
            redacted: false,
            in_reply_to: None,
            replace: None,
            extra_content: None,
        }
    }

    /// Generates an unique transaction id for this message
    /// The txn_id is generated using the md5sum of a concatenation of the message room id, the
    /// message body and the date.
    ///
    /// https://matrix.org/docs/spec/client_server/r0.3.0.html#put-matrix-client-r0-rooms-roomid-send-eventtype-txnid
    pub fn get_txn_id(&self) -> String {
        let msg_str = format!("{}{}{}", self.room, self.body, self.date);
        let digest = md5::compute(msg_str.as_bytes());
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

        let id: EventId = msg["event_id"].as_str().unwrap_or_default().try_into()?;
        let type_ = msg["type"].as_str().unwrap_or_default();

        let redacted = msg["unsigned"].get("redacted_because") != None;

        let mut message = Message {
            sender,
            date: server_timestamp,
            room: room_id.clone(),
            // It is mandatory for a message event to have
            // an event_id field
            id: Some(id),
            mtype: type_.to_string(),
            body: String::new(),
            url: None,
            local_path: None,
            thumb: None,
            local_path_thumb: None,
            formatted_body: None,
            format: None,
            source: serde_json::to_string_pretty(&msg).ok(),
            receipt: HashMap::new(),
            redacted,
            in_reply_to: None,
            replace: None,
            extra_content: None,
        };

        let c = &msg["content"];
        match type_ {
            "m.room.message" => message.parse_m_room_message(c),
            "m.sticker" => message.parse_m_sticker(c),
            _ => {}
        };

        Ok(message)
    }

    fn parse_m_room_message(&mut self, mut c: &JsonValue) {
        let rel_type = c["m.relates_to"]["rel_type"]
            .as_str()
            .map(String::from)
            .unwrap_or_default();

        if rel_type == "m.replace" {
            self.replace = c["m.relates_to"]["event_id"]
                .as_str()
                .and_then(|evid| evid.try_into().ok());
            c = &c["m.new_content"];
        }

        let mtype = c["msgtype"].as_str().map(String::from).unwrap_or_default();
        let body = c["body"].as_str().map(String::from).unwrap_or_default();
        let formatted_body = c["formatted_body"].as_str().map(String::from);
        let format = c["format"].as_str().map(String::from);

        match mtype.as_str() {
            "m.image" | "m.file" | "m.video" | "m.audio" => {
                self.url = c["url"].as_str().map(Url::parse).and_then(Result::ok);
                self.thumb = c["info"]["thumbnail_url"]
                    .as_str()
                    .map(Url::parse)
                    .and_then(Result::ok)
                    .or_else(|| Some(self.url.clone()?));
            }
            "m.text" => {
                // Only m.text messages can be replies for backward compatibility
                // https://matrix.org/docs/spec/client_server/r0.4.0.html#rich-replies
                self.in_reply_to = c["m.relates_to"]["m.in_reply_to"]["event_id"]
                    .as_str()
                    .and_then(|evid| evid.try_into().ok());
            }
            _ => {}
        };

        self.mtype = mtype;
        self.body = body;
        self.formatted_body = formatted_body;
        self.format = format;
    }

    fn parse_m_sticker(&mut self, c: &JsonValue) {
        self.url = c["url"].as_str().map(Url::parse).and_then(Result::ok);
        self.thumb = c["info"]["thumbnail_url"]
            .as_str()
            .map(Url::parse)
            .and_then(Result::ok)
            .or_else(|| Some(self.url.clone()?));
        self.body = c["body"].as_str().map(String::from).unwrap_or_default();
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

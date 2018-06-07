extern crate md5;
extern crate chrono;
extern crate serde_json;
extern crate time;
use self::chrono::prelude::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::str::FromStr;
use self::serde_json::Value as JsonValue;
use self::time::Duration;

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
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
}

impl Clone for Message {
    fn clone(&self) -> Message {
        Message {
            sender: self.sender.clone(),
            mtype: self.mtype.clone(),
            body: self.body.clone(),
            date: self.date.clone(),
            room: self.room.clone(),
            thumb: self.thumb.clone(),
            url: self.url.clone(),
            id: self.id.clone(),
            formatted_body: self.formatted_body.clone(),
            format: self.format.clone(),
            source: self.source.clone(),
            receipt: self.receipt.clone(),
            redacted: self.redacted,
        }
    }
}

impl Default for Message {
    fn default() -> Message {
        Message {
            sender: String::new(),
            mtype: String::from("m.text"),
            body: String::from("default"),
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
        }
    }
}

impl PartialEq for Message {
    fn eq(&self, other: &Message) -> bool {
        match (self.id.clone(), other.id.clone()) {
            (Some(self_id), Some(other_id)) => self_id == other_id,
            _ => self.sender == other.sender && self.body == other.body,
        }
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
    pub fn types() -> [&'static str; 3] {
        [
            "m.room.message",
            "m.room.member",
            "m.sticker",
        ]
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
    pub fn parse_room_message(roomid: String, msg: &JsonValue) -> Message {
        let sender = msg["sender"].as_str().unwrap_or("");
        let mut age = msg["age"].as_i64().unwrap_or(0);
        if age == 0 {
            age = msg["unsigned"]["age"].as_i64().unwrap_or(0);
        }

        let id = msg["event_id"].as_str().unwrap_or("");
        let type_ = msg["type"].as_str().unwrap_or("");

        let redacted = msg["unsigned"].get("redacted_because") != None;

        let mut message = Message {
            sender: sender.to_string(),
            date: Message::age_to_datetime(age),
            room: roomid.clone(),
            id: Some(id.to_string()),
            mtype: type_.to_string(),
            body: "".to_string(),
            url: None,
            thumb: None,
            formatted_body: None,
            format: None,
            source: serde_json::to_string_pretty(&msg).ok(),
            receipt: HashMap::new(),
            redacted,
        };

        let c = &msg["content"];
        match type_ {
            "m.room.message" => Message::parse_m_room_message(&mut message, c),
            "m.room.member" => Message::parse_m_room_member(&mut message, c),
            "m.sticker" => Message::parse_m_sticker(&mut message, c),
            _ => {}
        };

        message
    }

    fn parse_m_room_message(msg: &mut Message, c: &JsonValue) {
        let mtype = c["msgtype"].as_str().unwrap_or("");
        let body = c["body"].as_str().unwrap_or("");
        let formatted_body = c["formatted_body"].as_str().map(|s| String::from(s));
        let format = c["format"].as_str().map(|s| String::from(s));

        match mtype {
            "m.image" | "m.file" | "m.video" | "m.audio" => {
                let url = String::from(c["url"].as_str().unwrap_or(""));
                let mut t = String::from(c["info"]["thumbnail_url"].as_str().unwrap_or(""));
                if t.is_empty() && !url.is_empty() {
                    t = url.clone();
                }

                msg.url = Some(url);
                msg.thumb = Some(t);
            }
            _ => {}
        };

        msg.mtype = mtype.to_string();
        msg.body = body.to_string();
        msg.formatted_body = formatted_body;
        msg.format = format;
    }

    fn parse_m_room_member(msg: &mut Message, c: &JsonValue) {
        let membership = c["membership"].as_str().unwrap_or("");
        let displayname = c["displayname"].as_str().unwrap_or(&msg.sender);

        let mut action: String = String::new();
        match membership {
            "join" => {
                action = String::from_str(displayname).unwrap();
                action.push_str(" joined");
            },
            "leave" => {
                action = String::from_str(displayname).unwrap();
                action.push_str(" left");
            },
            "invite" => {
                action = String::from_str(displayname).unwrap();
                action.push_str(" was invited");
            },
            _ => {}
        }

        msg.body = action;
    }

    fn parse_m_sticker(msg: &mut Message, c: &JsonValue) {
        let body = c["body"].as_str().unwrap_or("");

        let url = String::from(c["url"].as_str().unwrap_or(""));
        let mut t = String::from(c["info"]["thumbnail_url"].as_str().unwrap_or(""));
        if t.is_empty() && !url.is_empty() {
            t = url.clone();
        }

        msg.body = body.to_string();
        msg.url = Some(url);
        msg.thumb = Some(t);
    }

    /// Create a vec of Message from a json event list
    ///
    /// * `roomid` - The messages room id
    /// * `events` - An iterator to the json events
    pub fn from_json_events_iter<'a, I>(roomid: String, events: I) -> Vec<Message>
        where I: Iterator<Item=&'a JsonValue> {
        let mut ms = vec![];

        let evs = events.filter(Message::supported_event);
        for msg in evs {
            let m = Message::parse_room_message(roomid.clone(), msg);
            ms.push(m);
        }

        ms
    }

    fn age_to_datetime(age: i64) -> DateTime<Local> {
        let now = Local::now();
        let diff = Duration::seconds(age / 1000);
        now - diff
    }

    pub fn set_receipt(&mut self, receipt: HashMap<String, i64>) {
        self.receipt = receipt;
    }
}

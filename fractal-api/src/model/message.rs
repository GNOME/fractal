extern crate md5;
extern crate chrono;
extern crate serde_json;
extern crate time;
use self::chrono::prelude::*;
use std::cmp::Ordering;
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
    pub fn types() -> [&'static str; 1] {
        [
            "m.room.message",
            //"m.sticker",
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

        let c = &msg["content"];
        let mtype = c["msgtype"].as_str().unwrap_or("");
        let body = c["body"].as_str().unwrap_or("");
        let formatted_body = c["formatted_body"].as_str().map(|s| String::from(s));
        let format = c["format"].as_str().map(|s| String::from(s));
        let mut url = String::new();
        let mut thumb = String::new();

        match mtype {
            "m.image" | "m.file" | "m.video" | "m.audio" => {
                url = String::from(c["url"].as_str().unwrap_or(""));
                let mut t = String::from(c["info"]["thumbnail_url"].as_str().unwrap_or(""));
                if t.is_empty() && !url.is_empty() {
                    t = url.clone();
                }
                thumb = t;
            }
            _ => {}
        };

        Message {
            sender: String::from(sender),
            mtype: String::from(mtype),
            body: String::from(body),
            date: Message::age_to_datetime(age),
            room: roomid.clone(),
            url: Some(url),
            thumb: Some(thumb),
            id: Some(String::from(id)),
            formatted_body: formatted_body,
            format: format,
        }
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
}

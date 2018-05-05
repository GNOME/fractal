extern crate md5;
extern crate chrono;
use self::chrono::prelude::*;

#[derive(Debug)]
#[derive(PartialOrd)]
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
        match self.id {
            Some(ref self_id) => {
                match other.id {
                    Some(ref other_id) => self_id == other_id,
                    None => self.sender == other.sender && self.body == other.body
                }
            },
            None => self.sender == other.sender && self.body == other.body
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
}

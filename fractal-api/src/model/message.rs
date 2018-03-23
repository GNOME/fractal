extern crate chrono;
use self::chrono::prelude::*;

#[derive(Debug)]
#[derive(PartialEq, PartialOrd)]
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

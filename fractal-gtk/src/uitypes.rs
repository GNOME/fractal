use crate::types::Message;
use crate::widgets;
use chrono::prelude::DateTime;
use chrono::prelude::Local;
use fractal_api::identifiers::{EventId, UserId};

/* MessageContent contains all data needed to display one row
 * therefore it should contain only one Message body with one format
 * To-Do: this should be moved to a file collecting all structs used in the UI */
#[derive(Debug, Clone)]
pub struct MessageContent {
    pub id: Option<EventId>,
    pub sender: UserId,
    pub sender_name: Option<String>,
    pub mtype: RowType,
    pub body: String,
    pub date: DateTime<Local>,
    pub thumb: Option<String>,
    pub url: Option<String>,
    pub formatted_body: Option<String>,
    pub format: Option<String>,
    /* in some places we still need the backend message type (e.g. media viewer) */
    pub msg: Message,
    pub highlights: Vec<String>,
    pub redactable: bool,
    pub last_viewed: bool,
    pub widget: Option<widgets::MessageBox>,
}

/* To-Do: this should be moved to a file collecting all structs used in the UI */
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RowType {
    #[allow(dead_code)]
    Divider,
    Mention,
    Emote,
    Message,
    Sticker,
    Image,
    Audio,
    Video,
    File,
    Emoji,
}

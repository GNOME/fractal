use comrak::{markdown_to_html, ComrakOptions};
use gio::prelude::{FileExt, FileInfoExt};
use gdk_pixbuf::Pixbuf;
use gdk_pixbuf::PixbufExt;
use gtk;
use gtk::prelude::*;
use lazy_static::lazy_static;
use log::error;
use serde_json::json;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::PathBuf;

use crate::appop::room::Force;
use crate::appop::AppOp;
use crate::App;

use crate::backend::BKCommand;
use crate::uitypes::MessageContent;
use crate::uitypes::RowType;
use crate::widgets;

use crate::types::Message;

pub struct TmpMsg {
    pub msg: Message,
    pub widget: Option<gtk::Widget>,
}

impl AppOp {
    pub fn get_message_by_id(&self, room_id: &str, id: &str) -> Option<Message> {
        let room = self.rooms.get(room_id)?;
        room.messages
            .iter()
            .find(|m| m.id == id.to_string())
            .cloned()
    }

    /// This function is used to mark as read the last message of a room when the focus comes in,
    /// so we need to force the mark_as_read because the window isn't active yet
    pub fn mark_active_room_messages(&mut self) {
        self.mark_last_message_as_read(Force(true));
    }

    pub fn add_room_message(&mut self, msg: &Message) -> Option<()> {
        if let Some(ui_msg) = self.create_new_room_message(msg) {
            if let Some(ref mut history) = self.history {
                history.add_new_message(ui_msg);
            }
        }
        None
    }

    pub fn add_tmp_room_message(&mut self, msg: Message) -> Option<()> {
        let messages = self.history.as_ref()?.get_listbox();
        if let Some(ui_msg) = self.create_new_room_message(&msg) {
            let backend = self.backend.clone();
            let mb = widgets::MessageBox::new(backend).tmpwidget(&ui_msg)?;
            let m = mb.get_listbox_row()?;
            messages.add(m);

            if let Some(w) = messages.get_children().iter().last() {
                self.msg_queue.insert(
                    0,
                    TmpMsg {
                        msg: msg.clone(),
                        widget: Some(w.clone()),
                    },
                );
            };
        }
        None
    }

    pub fn clear_tmp_msgs(&mut self) {
        for t in self.msg_queue.iter_mut() {
            if let Some(ref w) = t.widget {
                w.destroy();
            }
            t.widget = None;
        }
    }

    pub fn append_tmp_msgs(&mut self) -> Option<()> {
        let messages = self.history.as_ref()?.get_listbox();

        let r = self.rooms.get(self.active_room.as_ref()?)?;
        let mut widgets = vec![];
        for t in self.msg_queue.iter().rev().filter(|m| m.msg.room == r.id) {
            if let Some(ui_msg) = self.create_new_room_message(&t.msg) {
                let backend = self.backend.clone();
                let mb = widgets::MessageBox::new(backend).tmpwidget(&ui_msg)?;
                let m = mb.get_listbox_row()?;
                messages.add(m);

                if let Some(w) = messages.get_children().iter().last() {
                    widgets.push(w.clone());
                }
            }
        }

        for (t, w) in self.msg_queue.iter_mut().rev().zip(widgets.iter()) {
            t.widget = Some(w.clone());
        }
        None
    }

    pub fn mark_last_message_as_read(&mut self, Force(force): Force) -> Option<()> {
        let window: gtk::Window = self
            .ui
            .builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");
        if window.is_active() || force {
            /* Move the last viewed mark to the last message */
            let active_room_id = self.active_room.as_ref()?;
            let room = self.rooms.get_mut(active_room_id)?;
            let uid = self.uid.clone()?;
            room.messages.iter_mut().for_each(|msg| {
                if msg.receipt.contains_key(&uid) {
                    msg.receipt.remove(&uid);
                }
            });
            let last_message = room.messages.last_mut()?;
            last_message.receipt.insert(self.uid.clone()?, 0);

            self.backend
                .send(BKCommand::MarkAsRead(
                    last_message.room.clone(),
                    last_message.id.clone(),
                ))
                .unwrap();
        }
        None
    }

    pub fn msg_sent(&mut self, _txid: String, evid: String) {
        if let Some(ref mut m) = self.msg_queue.pop() {
            if let Some(ref w) = m.widget {
                w.destroy();
            }
            m.widget = None;
            m.msg.id = evid;
            self.show_room_messages(vec![m.msg.clone()]);
        }
        self.force_dequeue_message();
    }

    pub fn retry_send(&mut self) {
        gtk::timeout_add(5000, move || {
            /* This will be removed once tmp messages are refactored */
            APPOP!(force_dequeue_message);
            gtk::Continue(false)
        });
    }

    pub fn force_dequeue_message(&mut self) {
        self.sending_message = false;
        self.dequeue_message();
    }

    pub fn dequeue_message(&mut self) {
        if self.sending_message {
            return;
        }

        self.sending_message = true;
        if let Some(next) = self.msg_queue.last() {
            let msg = next.msg.clone();
            match &next.msg.mtype[..] {
                "m.image" | "m.file" => {
                    self.backend.send(BKCommand::AttachFile(msg)).unwrap();
                }
                _ => {
                    self.backend.send(BKCommand::SendMsg(msg)).unwrap();
                }
            }
        } else {
            self.sending_message = false;
        }
    }

    pub fn send_message(&mut self, msg: String) {
        if msg.is_empty() {
            // Not sending empty messages
            return;
        }

        if let Some(room) = self.active_room.clone() {
            if let Some(sender) = self.uid.clone() {
                let body = msg.clone();
                let mtype = String::from("m.text");
                let mut m = Message::new(room, sender, body, mtype);

                if msg.starts_with("/me ") {
                    m.body = msg.trim_left_matches("/me ").to_owned();
                    m.mtype = String::from("m.emote");
                }

                // Riot does not properly show emotes with Markdown;
                // Emotes with markdown have a newline after the username
                if m.mtype != "m.emote" && self.md_enabled {
                    let mut md_parsed_msg = markdown_to_html(&msg, &ComrakOptions::default());

                    // Removing wrap tag: <p>..</p>\n
                    let limit = md_parsed_msg.len() - 5;
                    let trim = match (md_parsed_msg.get(0..3), md_parsed_msg.get(limit..)) {
                        (Some(open), Some(close)) if open == "<p>" && close == "</p>\n" => true,
                        _ => false,
                    };
                    if trim {
                        md_parsed_msg = md_parsed_msg
                            .get(3..limit)
                            .unwrap_or(&md_parsed_msg)
                            .to_string();
                    }

                    if md_parsed_msg != msg {
                        m.formatted_body = Some(md_parsed_msg);
                        m.format = Some(String::from("org.matrix.custom.html"));
                    }
                }

                self.add_tmp_room_message(m);
                self.dequeue_message();
            } else {
                error!("Can't send message: No user is logged in");
            }
        } else {
            error!("Can't send message: No active room");
        }
    }

    pub fn attach_message(&mut self, path: PathBuf) {
        if let Some(room) = self.active_room.clone() {
            if let Some(sender) = self.uid.clone() {
                if let Ok(info) = gio::File::new_for_path(&path).query_info(
                    &gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                    gio::FileQueryInfoFlags::NONE,
                    None,
                ) {
                    // This should always return a type
                    let mime = info
                        .get_content_type()
                        .expect("Could not parse content type from file");
                    let mtype = match mime.as_ref() {
                        "image/gif" => "m.image",
                        "image/png" => "m.image",
                        "image/jpeg" => "m.image",
                        "image/jpg" => "m.image",
                        _ => "m.file",
                    };
                    let body = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default();
                    let path_string = path.to_str().unwrap_or_default();

                    let mut m = Message::new(room, sender, body.to_string(), mtype.to_string());
                    if mtype == "m.image" {
                        m.extra_content = get_image_media_info(path_string, mime.as_ref());
                    }
                    m.url = Some(path_string.to_string());
                    self.add_tmp_room_message(m);
                    self.dequeue_message();
                } else {
                    error!("Can't send message: Could not query info");
                }
            } else {
                error!("Can't send message: No user is logged in");
            }
        } else {
            error!("Can't send message: No active room");
        }
    }

    /// This method is called when a tmp message with an attach is sent correctly
    /// to the matrix media server and we've the real url to use so we can
    /// replace the tmp message with the same id with this new one
    pub fn attached_file(&mut self, msg: Message) {
        let p = self.msg_queue.iter().position(|m| m.msg == msg);
        if let Some(i) = p {
            let w = self.msg_queue.remove(i);
            w.widget.map(|w| w.destroy());
        }
        self.add_tmp_room_message(msg);
    }

    /* TODO: find a better name for this function */
    pub fn show_room_messages(&mut self, newmsgs: Vec<Message>) -> Option<()> {
        let mut msgs = vec![];

        for msg in newmsgs.iter() {
            if let Some(r) = self.rooms.get_mut(&msg.room) {
                if !r.messages.contains(msg) {
                    r.messages.push(msg.clone());
                    msgs.push(msg.clone());
                }
            }
        }

        let mut msg_in_active = false;
        let uid = self.uid.clone()?;
        for msg in msgs.iter() {
            let should_notify = msg.sender != uid
                && (msg.body.contains(&self.username.clone()?)
                    || self.rooms.get(&msg.room).map_or(false, |r| r.direct));

            if should_notify {
                let window: gtk::Window = self
                    .ui
                    .builder
                    .get_object("main_window")
                    .expect("Can't find main_window in ui file.");
                if let Some(app) = window.get_application() {
                    self.notify(app, &msg.room, &msg.id);
                }
            }

            if !msg.redacted && self.active_room.as_ref().map_or(false, |x| x == &msg.room) {
                self.add_room_message(&msg);
                msg_in_active = true;
            }

            self.roomlist.moveup(msg.room.clone());
            self.roomlist.set_bold(msg.room.clone(), true);
        }

        if msg_in_active {
            self.mark_last_message_as_read(Force(false));
        }

        None
    }

    /* TODO: find a better name for this function */
    pub fn show_room_messages_top(
        &mut self,
        msgs: Vec<Message>,
        roomid: String,
        prev_batch: Option<String>,
    ) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.prev_batch = prev_batch;
        }

        let active_room = self.active_room.clone().unwrap_or_default();
        let mut list = vec![];
        for item in msgs.iter().rev() {
            /* create a list of new messages to load to the history */
            if item.room == active_room && !item.redacted {
                if let Some(ui_msg) = self.create_new_room_message(item) {
                    list.push(ui_msg);
                }
            }

            if let Some(r) = self.rooms.get_mut(&item.room) {
                r.messages.insert(0, item.clone());
            }
        }

        if let Some(ref mut history) = self.history {
            history.add_old_messages_in_batch(list);
        }
    }

    /* parese a backend Message into a Message for the UI */
    pub fn create_new_room_message(&self, msg: &Message) -> Option<MessageContent> {
        let mut highlights = vec![];
        lazy_static! {
            static ref emoji_regex: regex::Regex = regex::Regex::new(r"(?x)
                ^
                [\p{White_Space}\p{Emoji}\p{Emoji_Presentation}\p{Emoji_Modifier}\p{Emoji_Modifier_Base}\p{Emoji_Component}]*
                [\p{Emoji}]+
                [\p{White_Space}\p{Emoji}\p{Emoji_Presentation}\p{Emoji_Modifier}\p{Emoji_Modifier_Base}\p{Emoji_Component}]*
                $
                # That string is made of at least one emoji, possibly more, possibly with modifiers, possibly with spaces, but nothing else
                ").unwrap();
        }

        let t = match msg.mtype.as_ref() {
            "m.emote" => RowType::Emote,
            "m.image" => RowType::Image,
            "m.sticker" => RowType::Sticker,
            "m.audio" => RowType::Audio,
            "m.video" => RowType::Video,
            "m.file" => RowType::File,
            _ => {
                /* set message type to mention if the body contains the username, we should
                 * also match for MXID */
                let is_mention = if let Some(user) = self.username.clone() {
                    msg.sender != self.uid.clone()? && msg.body.contains(&user)
                } else {
                    false
                };

                if is_mention {
                    if let Some(user) = self.username.clone() {
                        highlights.push(user);
                    }
                    if let Some(mxid) = self.uid.clone() {
                        highlights.push(mxid);
                    }
                    highlights.push(String::from("message_menu"));

                    RowType::Mention
                } else if emoji_regex.is_match(&msg.body) {
                    RowType::Emoji
                } else {
                    RowType::Message
                }
            }
        };

        let room = self.rooms.get(&msg.room)?;
        let name = if let Some(member) = room.members.get(&msg.sender) {
            member.alias.clone()
        } else {
            None
        };

        let uid = self.uid.clone().unwrap_or_default();
        let power_level = match self.uid.clone().and_then(|uid| room.power_levels.get(&uid)) {
            Some(&pl) => pl,
            None => 0,
        };
        let redactable = power_level != 0 || uid == msg.sender;

        let is_last_viewed = msg.receipt.contains_key(&uid);
        Some(create_ui_message(
            msg.clone(),
            name,
            t,
            highlights,
            redactable,
            is_last_viewed,
        ))
    }
}

/* FIXME: don't convert msg to ui messages here, we should later get a ui message from storage */
fn create_ui_message(
    msg: Message,
    name: Option<String>,
    t: RowType,
    highlights: Vec<String>,
    redactable: bool,
    last_viewed: bool,
) -> MessageContent {
    MessageContent {
        msg: msg.clone(),
        id: msg.id,
        sender: msg.sender,
        sender_name: name,
        mtype: t,
        body: msg.body,
        date: msg.date,
        thumb: msg.thumb,
        url: msg.url,
        formatted_body: msg.formatted_body,
        format: msg.format,
        last_viewed: last_viewed,
        highlights: highlights,
        redactable,
        widget: None,
    }
}

/// This function opens the image, creates a thumbnail
/// and populates the info Json with the information it has

fn get_image_media_info(file: &str, mimetype: &str) -> Option<JsonValue> {
    let (_, w, h) = Pixbuf::get_file_info(&file)?;
    let size = fs::metadata(&file).ok()?.len();

    // make thumbnail max 800x600
    let thumb = Pixbuf::new_from_file_at_scale(&file, 800, 600, true).ok()?;
    thumb.savev("/tmp/fractal_thumb.png", "png", &[]).ok()?;

    let info = json!({
        "info": {
            "thumbnail_url": "",
            "thumbnail_info": {
                "w": thumb.get_width(),
                "h": thumb.get_height(),
                "mimetype": "image/png"
            },
            "w": w,
            "h": h,
            "size": size,
            "mimetype": mimetype,
            "orientation": 0
        }
    });

    Some(info)
}

use chrono::prelude::*;
use comrak::{markdown_to_html, ComrakOptions};
use gstreamer_editing_services::UriClipAsset;
use gstreamer_editing_services::UriClipAssetExt;
use gtk;
use gtk::prelude::*;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tree_magic;

use crate::appop::room::Force;
use crate::appop::AppOp;
use crate::App;

use crate::backend::BKCommand;
use crate::uitypes::MessageContent;
use crate::uitypes::RowType;
use crate::widgets;

use crate::types::Message;
use gdk_pixbuf::Pixbuf;
use serde_json::json;
use serde_json::Value as JsonValue;

pub struct TmpMsg {
    pub msg: Message,
    pub widget: Option<gtk::Widget>,
}

impl AppOp {
    pub fn get_message_by_id(&self, room_id: &str, id: &str) -> Option<Message> {
        let room = self.rooms.get(room_id)?;
        room.messages
            .iter()
            .find(|m| m.id == Some(id.to_string()))
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
                    last_message.id.clone()?,
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
            m.msg.id = Some(evid);
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
                "m.image" | "m.file" | "m.audio" | "m.video" => {
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

        let room = self.active_room.clone();
        let now = Local::now();

        let mtype = String::from("m.text");

        let mut m = Message {
            sender: self.uid.clone().unwrap_or_default(),
            mtype: mtype,
            body: msg.clone(),
            room: room.clone().unwrap_or_default(),
            date: now,
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
        };

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

        m.id = Some(m.get_txn_id());
        self.add_tmp_room_message(m.clone());
        self.dequeue_message();
    }

    pub fn attach_message(&mut self, file: String) -> Message {
        let now = Local::now();
        let room = self.active_room.clone();
        let f = file.clone();
        let p: &Path = Path::new(&f);
        let mime = tree_magic::from_filepath(p);
        let mtype = match mime.as_ref() {
            "image/gif" => "m.image",
            "image/png" => "m.image",
            "image/jpeg" => "m.image",
            "image/jpg" => "m.image",
            "audio/mp4" => "m.audio",
            "audio/webm" => "m.audio",
            "audio/aac" => "m.audio",
            "audio/mpeg" => "m.audio",
            "audio/ogg" => "m.audio",
            "audio/wave" => "m.audio",
            "audio/wav" => "m.audio",
            "audio/x-wav" => "m.audio",
            "audio/x-pn-wav" => "m.audio",
            "audio/flac" => "m.audio",
            "audio/x-flac" => "m.audio",
            "application/x-riff" => "m.audio",
            "video/mp4" => "m.video",
            "video/web" => "m.video",
            "video/ogg" => "m.video",
            _ => "m.file",
        };
        let body = String::from(file.split("/").last().unwrap_or(&file));

        let info = match mtype {
            "m.image" => get_image_media_info(&file, mime.as_ref()),
            "m.audio" => get_audio_media_info(&file, mime.as_ref()),
            "m.video" => get_video_media_info(&file, mime.as_ref()),
            _ => None,
        };

        let mut m = Message {
            sender: self.uid.clone().unwrap_or_default(),
            mtype: mtype.to_string(),
            body: body,
            room: room.unwrap_or_default(),
            date: now,
            thumb: None,
            url: Some(file),
            id: None,
            formatted_body: None,
            format: None,
            source: None,
            receipt: HashMap::new(),
            redacted: false,
            in_reply_to: None,
            extra_content: info,
        };

        m.id = Some(m.get_txn_id());
        self.add_tmp_room_message(m.clone());
        self.dequeue_message();

        m
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

    pub fn attach_file(&mut self) {
        let window: gtk::ApplicationWindow = self
            .ui
            .builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");

        let file_chooser = gtk::FileChooserNative::new(
            None,
            Some(&window),
            gtk::FileChooserAction::Open,
            None,
            None,
        );

        // Running in a *thread* to free self lock
        // FIXME don't use idle_add
        gtk::idle_add(move || {
            let result = file_chooser.run();
            if gtk::ResponseType::from(result) == gtk::ResponseType::Accept {
                if let Some(fname) = file_chooser.get_filename() {
                    let f = String::from(fname.to_str().unwrap_or_default());
                    APPOP!(attach_message, (f));
                }
            }
            gtk::Continue(false)
        });
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
                if let Some(ref id) = msg.id {
                    let window: gtk::Window = self
                        .ui
                        .builder
                        .get_object("main_window")
                        .expect("Can't find main_window in ui file.");
                    if let Some(app) = window.get_application() {
                        self.notify(app, &msg.room, id);
                    }
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
        id: msg.id.unwrap_or_default(),
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

/// This function open the image and fill the info data as a Json value
/// If something fails this will returns None
///
/// The output json will look like:
///
/// {
///  "info": {
///   "h": 296,
///   "w": 296,
///   "size": 8796,
///   "orientation": 0,
///   "mimetype": "image/png"
///  }
/// }
fn get_image_media_info(file: &str, mimetype: &str) -> Option<JsonValue> {
    let (_, w, h) = Pixbuf::get_file_info(file)?;
    let size = fs::metadata(file).ok()?.len();

    let info = json!({
        "info": {
            "w": w,
            "h": h,
            "size": size,
            "mimetype": mimetype,
            "orientation": 0,
        }
    });

    Some(info)
}

fn get_audio_media_info(file: &str, mimetype: &str) -> Option<JsonValue> {
    let nfile = format!("file://{}", file);
    let uri = UriClipAsset::request_sync(&nfile).ok()?;
    let duration = uri?.get_duration().mseconds()?;
    let size = fs::metadata(file).ok()?.len();

    let info = json!({
        "info": {
            "size": size,
            "mimetype": mimetype,
            "duration": duration,
        }
    });

    Some(info)
}

fn get_video_media_info(file: &str, mimetype: &str) -> Option<JsonValue> {
    let nfile = format!("file://{}", file);
    let uri = UriClipAsset::request_sync(&nfile).ok()?;
    let duration = uri?.get_duration().mseconds()?;
    let size = fs::metadata(file).ok()?.len();

    let info = json!({
        "info": {
            "size": size,
            "mimetype": mimetype,
            "duration": duration,
        // Still have to figure out how to populate the thumbnail metadata
        // "thumbnail_info": {
        //         "h": 300,
        //         "mimetype": "image/jpeg",
        //         "size": 46144,
        //         "w": 300
        //     },
        //     "thumbnail_url": "mxc://localhost/FHyPlCeYUSFFxlgbQYZmoEoe",
        //     "w": 480,
        }
    });

    Some(info)
}

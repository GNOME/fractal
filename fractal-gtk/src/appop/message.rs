use tree_magic;
use std::fs;
use std::path::Path;
use std::collections::HashMap;
use gtk;
use gtk::prelude::*;
use chrono::prelude::*;
use comrak::{markdown_to_html, ComrakOptions};

use app::InternalCommand;
use appop::AppOp;
use app::App;
use appop::room::Force;

use glib;
use widgets;
use uitypes::MessageContent;
use uitypes::RowType;
use backend::BKCommand;

use types::Message;
use serde_json::Value as JsonValue;
use gdk_pixbuf::Pixbuf;

#[derive(Debug, Clone)]
pub enum MsgPos {
    Top,
    Bottom,
}

pub struct TmpMsg {
    pub msg: Message,
    pub widget: Option<gtk::Widget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LastViewed {
    Inline,
    Last,
    No,
}

impl AppOp {
    /// This function is used to mark as read the last message of a room when the focus comes in,
    /// so we need to force the mark_as_read because the window isn't active yet
    pub fn mark_active_room_messages(&mut self) {
        let mut msg: Option<Message> = None;

        if let Some(ref active_room_id) = self.active_room {
            if let Some(ref r) = self.rooms.get(active_room_id) {
                if let Some(m) = r.messages.last() {
                    msg = Some(m.clone());
                }
            }
        }

        // this is done here because in the above we've a reference to self and mark as read needs
        // a mutable reference to self so we can't do it inside
        if let Some(m) = msg {
            self.mark_as_read(&m, Force(true));
        }
    }

    pub fn is_last_viewed(&self, msg: &Message) -> LastViewed {
        match self.last_viewed_messages.get(&msg.room) {
            Some(lvm_id) if msg.id.clone().map_or(false, |id| *lvm_id == id) => {
                match self.rooms.get(&msg.room) {
                    Some(r) => {
                        match r.messages.last() {
                            Some(m) if m == msg => LastViewed::Last,
                            _ => LastViewed::Inline,
                        }
                    },
                    _ => LastViewed::Inline,
                }
            },
            _ => LastViewed::No,
        }
    }

    pub fn get_first_new_from_last(&self, last_msg: &Message) -> Option<Message> {
        match self.is_last_viewed(last_msg) {
            LastViewed::Last | LastViewed::No => None,
            LastViewed::Inline => {
                self.rooms.get(&last_msg.room).and_then(|r| {
                    r.messages.clone().into_iter()
                              .filter(|msg| *msg > *last_msg && msg.sender !=
                                      self.uid.clone().unwrap_or_default()).next()
                })
            }
        }
    }

    pub fn get_msg_from_id(&self, roomid: &str, msg_id: &str) -> Option<Message> {
        let room = self.rooms.get(roomid);

        room.and_then(|r| r.messages.clone().into_iter()
                                    .filter(|msg| msg.id.clone().unwrap_or_default() == msg_id)
                                    .next())
    }

    pub fn is_first_new(&self, msg: &Message) -> bool {
        match self.first_new_messages.get(&msg.room) {
            None => false,
            Some(new_msg) => {
                match new_msg {
                    None => false,
                    Some(new_msg) => new_msg == msg,
                }
            }
        }
    }

    pub fn add_room_message(&mut self,
                            msg: Message,
                            msgpos: MsgPos,
                            first_new: bool) {
        if msg.room == self.active_room.clone().unwrap_or_default() && !msg.redacted {
            if let Some(ui_msg) = self.create_new_room_message(&msg) {
                if let Some(ref mut history) = self.history {
                    match msgpos {
                        MsgPos::Bottom => {
                            if first_new {
                                history.add_divider();
                            }
                            history.add_new_message(ui_msg);
                        },
                        MsgPos::Top => {
                            history.add_old_message(ui_msg);
                        }
                    }

                }
            }
        }
    }

    pub fn add_tmp_room_message(&mut self, msg: Message) -> Option<()> {
        let messages = self.history.as_ref()?.get_listbox();
        if let Some(ui_msg) = self.create_new_room_message(&msg) {
            if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
                let backend = self.backend.clone();
                let ui = self.ui.clone();
                let mb = widgets::MessageBox::new(backend, ui).tmpwidget(&ui_msg)?;
                let m = mb.get_listbox_row()?;
                if let Some(ref image) = mb.image {
                    let msg = msg.clone();
                    let room = r.clone();
                    image.connect_button_press_event(move |_, btn| {
                        if btn.get_button() != 3 {
                            let msg = msg.clone();
                            let room = room.clone();
                            APPOP!(create_media_viewer, (msg, room));

                            Inhibit(true)
                        } else {
                            Inhibit(false)
                        }
                    });
                }
                messages.add(m);
            }

            if let Some(w) = messages.get_children().iter().last() {
                self.msg_queue.insert(0, TmpMsg {
                    msg: msg.clone(),
                    widget: Some(w.clone()),
                });
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
        let messages = self.message_box.clone();

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            let mut widgets = vec![];
            for t in self.msg_queue.iter().rev().filter(|m| m.msg.room == r.id) {
                if let Some(ui_msg) = self.create_new_room_message(&t.msg) {
                    let backend = self.backend.clone();
                    let ui = self.ui.clone();
                    let mb = widgets::MessageBox::new(backend, ui).tmpwidget(&ui_msg)?;
                    let m = mb.get_listbox_row()?;
                    if let Some(ref image) = mb.image {
                        info!("i have a image");
                        let msg = t.msg.clone();
                        let room = r.clone();
                        image.connect_button_press_event(move |_, btn| {
                            if btn.get_button() != 3 {
                                let msg = msg.clone();
                                let room = room.clone();
                                APPOP!(create_media_viewer, (msg, room));

                                Inhibit(true)
                            } else {
                                Inhibit(false)
                            }
                        });
                    }
                    messages.add(m);

                    if let Some(w) = messages.get_children().iter().last() {
                        widgets.push(w.clone());
                    }
                }
            }

            for (t, w) in self.msg_queue.iter_mut().rev().zip(widgets.iter()) {
                t.widget = Some(w.clone());
            }
        }
        None
    }

    pub fn set_last_viewed_messages(&mut self) {
        if let Some(uid) = self.uid.clone() {
            for room in self.rooms.values() {
                let roomid = room.id.clone();

                if !self.last_viewed_messages.contains_key(&roomid) {
                    if let Some(lvm) = room.messages.iter().filter(|msg| msg.receipt.contains_key(&uid) && msg.id.is_some()).next() {
                        self.last_viewed_messages.insert(roomid, lvm.id.clone().unwrap_or_default());
                    }
                }
            }
        }
    }

    pub fn mark_as_read(&mut self, msg: &Message, Force(force): Force) {
        let window: gtk::Window = self.ui.builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");
        if window.is_active() || force {
            self.last_viewed_messages.insert(msg.room.clone(), msg.id.clone().unwrap_or_default());
            self.backend.send(BKCommand::MarkAsRead(msg.room.clone(),
                                                    msg.id.clone().unwrap_or_default())).unwrap();
        }
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
        let tx = self.internal.clone();
        gtk::timeout_add(5000, move || {
            tx.send(InternalCommand::ForceDequeueMessage).unwrap();
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

        /* reenable autoscroll to jump to new message in history */
        self.autoscroll = true;

        // Riot does not properly show emotes with Markdown;
        // Emotes with markdown have a newline after the username
        if m.mtype != "m.emote" && self.md_enabled {
            let mut md_parsed_msg = markdown_to_html(&msg, &ComrakOptions::default());

            // Removing wrap tag: <p>..</p>\n
            let limit = md_parsed_msg.len() - 5;
            let trim = match (md_parsed_msg.get(0..3), md_parsed_msg.get(limit..)) {
                (Some(open), Some(close)) if open == "<p>" && close == "</p>\n" => { true }
                _ => { false }
            };
            if trim {
                md_parsed_msg = md_parsed_msg.get(3..limit).unwrap_or(&md_parsed_msg).to_string();
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
        /* reenable autoscroll to jump to new message in history */
        self.autoscroll = true;

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
            _ => "m.file"
        };
        let body = String::from(file.split("/").last().unwrap_or(&file));

        let info = match mtype {
            "m.image" => get_image_media_info(&file, mime.as_ref()),
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
        let window: gtk::ApplicationWindow = self.ui.builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");

        let file_chooser = gtk::FileChooserNative::new(
            None,
            Some(&window),
            gtk::FileChooserAction::Open,
            None,
            None,
        );

        let internal = self.internal.clone();
        // Running in a *thread* to free self lock
        gtk::idle_add(move || {
            let result = file_chooser.run();
            if gtk::ResponseType::from(result) == gtk::ResponseType::Accept {
                if let Some(fname) = file_chooser.get_filename() {
                    let f = String::from(fname.to_str().unwrap_or(""));
                    internal.send(InternalCommand::AttachMessage(f)).unwrap();
                }
            }
            gtk::Continue(false)
        });
    }

    pub fn load_more_messages(&mut self) -> Option<()> {
        if self.loading_more {
            return None;
        }

        self.loading_more = true;
        let loading_spinner = self.history.as_ref()?.get_loading_spinner();
        loading_spinner.start();

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            if let Some(prev_batch) = r.prev_batch.clone() {
                self.backend.send(BKCommand::GetRoomMessages(r.id.clone(), prev_batch)).unwrap();
            } else if let Some(msg) = r.messages.iter().next() {
                // no prev_batch so we use the last message to calculate that in the backend
                self.backend.send(BKCommand::GetRoomMessagesFromMsg(r.id.clone(), msg.clone())).unwrap();
            } else if let Some(from) = self.since.clone() {
                // no messages and no prev_batch so we use the last since
                self.backend.send(BKCommand::GetRoomMessages(r.id.clone(), from)).unwrap();
            } else {
                loading_spinner.stop();
                self.loading_more = false;
            }
        }
        None
    }

    pub fn load_more_normal(&mut self) -> Option<()> {
        self.history.as_ref()?.get_loading_spinner().stop();
        self.loading_more = false;
        None
    }

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

        let uid = self.uid.clone()?;
        for msg in msgs.iter() {
            let should_notify = msg.sender != uid &&
                                (msg.body.contains(&self.username.clone()?) ||
                                self.rooms.get(&msg.room).map_or(false, |r| r.direct));

            if should_notify {
                self.notify(msg);
            }

            let command = InternalCommand::AddRoomMessage(msg.clone(),
                                                          MsgPos::Bottom,
                                                          self.is_first_new(&msg));
            self.internal.send(command).unwrap();

            self.roomlist.moveup(msg.room.clone());
            self.roomlist.set_bold(msg.room.clone(), true);
        }

        if !msgs.is_empty() {
            let active_room = self.active_room.clone().unwrap_or_default();
            let fs = msgs.iter().filter(|x| x.room == active_room);
            if let Some(msg) = fs.last() {
                self.mark_as_read(msg, Force(false));
            }
        }

        Some(())
    }

    pub fn show_room_messages_top(&mut self, msgs: Vec<Message>, roomid: String, prev_batch: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.prev_batch = prev_batch;
        }

        if msgs.is_empty() {
            self.load_more_normal();
            return;
        }

        for msg in msgs.iter().rev() {
            if let Some(r) = self.rooms.get_mut(&msg.room) {
                r.messages.insert(0, msg.clone());
            }
        }

        let size = msgs.len() - 1;
        for i in 0..size+1 {
            let msg = &msgs[size - i];

            let command = InternalCommand::AddRoomMessage(msg.clone(),
                                                          MsgPos::Top,
                                                          self.is_first_new(&msg));
            self.internal.send(command).unwrap();

        }
        self.internal.send(InternalCommand::LoadMoreNormal).unwrap();
    }

    /* parese a backend Message into a Message for the UI */
    pub fn create_new_room_message(&self, msg: &Message) -> Option<MessageContent> {
        let mut highlights = vec![];
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

        Some(create_ui_message(msg.clone(), name, t, highlights, redactable))
    }
}

/* FIXME: don't convert msg to ui messages here, we should later get a ui message from storage */
fn create_ui_message (msg: Message, name: Option<String>, t: RowType, highlights: Vec<String>, redactable: bool) -> MessageContent {
        MessageContent {
        msg: msg.clone(),
        id: msg.id.unwrap_or(String::from("")),
        sender: msg.sender,
        sender_name: name,
        mtype: t,
        body: msg.body,
        date: msg.date,
        thumb: msg.thumb,
        url: msg.url,
        formatted_body: msg.formatted_body,
        format: msg.format,
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

extern crate comrak;
extern crate tree_magic;

use std::path::Path;
use std::collections::HashMap;

use gtk;
use gtk::prelude::*;
use chrono::prelude::*;
use self::comrak::{markdown_to_html, ComrakOptions};

use app::InternalCommand;
use appop::AppOp;
use app::App;
use appop::RoomPanel;
use appop::room::Force;

use glib;
use globals;
use widgets;
use uitypes::MessageContent;
use uitypes::RowType;
use backend::BKCommand;

use types::Message;


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

    /* FIXME: remove not used arguments */
    pub fn add_room_message(&mut self,
                            msg: Message,
                            msgpos: MsgPos,
                            _prev: Option<Message>,
                            _force_full: bool,
                            first_new: bool) {
        if let Some(mut history) = self.history.clone() {
            if msg.room == self.active_room.clone().unwrap_or_default() && !msg.redacted {
                if let Some(_r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
                    if let Some(ui_msg) = self.create_new_room_message(&msg) {
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
    }

    pub fn add_tmp_room_message(&mut self, msg: Message) {
        if let Some(ui_msg) = self.create_new_room_message(&msg) {
            let messages = self.ui.builder
                .get_object::<gtk::ListBox>("message_list")
                .expect("Can't find message_list in ui file.");

            if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
                let m;
                {
                    let backend = self.backend.clone();
                    let ui = self.ui.clone();
                    let mut mb = widgets::MessageBox::new(&ui_msg, backend, &ui);
                    m = mb.tmpwidget();
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
                }

                messages.add(&m);
            }

            if let Some(w) = messages.get_children().iter().last() {
                self.msg_queue.insert(0, TmpMsg {
                    msg: msg.clone(),
                    widget: Some(w.clone()),
                });
            };
        }
    }

    pub fn clear_tmp_msgs(&mut self) {
        for t in self.msg_queue.iter_mut() {
            if let Some(ref w) = t.widget {
                w.destroy();
            }
            t.widget = None;
        }
    }

    pub fn append_tmp_msgs(&mut self) {
        let messages = self.ui.builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            let mut widgets = vec![];
            for t in self.msg_queue.iter().rev().filter(|m| m.msg.room == r.id) {
                if let Some(ui_msg) = self.create_new_room_message(&t.msg) {
                    let m;
                    {
                        let backend = self.backend.clone();
                        let ui = self.ui.clone();
                        let mut mb = widgets::MessageBox::new(&ui_msg, backend, &ui);
                        m = mb.tmpwidget();
                        if let Some(ref image) = mb.image {
                            println!("i have a image");
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
                    }

                    messages.add(&m);
                    if let Some(w) = messages.get_children().iter().last() {
                        widgets.push(w.clone());
                    }
                }
            }

            for (t, w) in self.msg_queue.iter_mut().rev().zip(widgets.iter()) {
                t.widget = Some(w.clone());
            }
        }
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
            self.show_room_messages(vec![m.msg.clone()], false);
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

    pub fn load_more_messages(&mut self) {
        if self.loading_more {
            return;
        }

        self.loading_more = true;
        self.load_more_spn.start();

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            if self.shown_messages < r.messages.len() {
                let msgs = r.messages.iter().rev()
                                     .skip(self.shown_messages)
                                     .take(globals::INITIAL_MESSAGES)
                                     .collect::<Vec<&Message>>();
                for (i, msg) in msgs.iter().enumerate() {
                    let command = InternalCommand::AddRoomMessage((*msg).clone(),
                                                                  MsgPos::Top,
                                                                  None,
                                                                  i == msgs.len() - 1,
                                                                  self.is_first_new(&msg));
                    self.internal.send(command).unwrap();
                }
                self.internal.send(InternalCommand::LoadMoreNormal).unwrap();
            } else if let Some(m) = r.messages.get(0) {
                self.backend.send(BKCommand::GetMessageContext(m.clone())).unwrap();
            }
        }
    }

    pub fn load_more_normal(&mut self) {
        self.load_more_spn.stop();
        self.loading_more = false;
    }

    pub fn show_room_messages(&mut self, newmsgs: Vec<Message>, init: bool) -> Option<()> {
        let mut msgs = vec![];

        for msg in newmsgs.iter() {
            if let Some(r) = self.rooms.get_mut(&msg.room) {
                if !r.messages.contains(msg) {
                    r.messages.push(msg.clone());
                    msgs.push(msg.clone());
                }
            }
        }

        let mut prev = None;
        for msg in msgs.iter() {
            let mut should_notify = msg.body.contains(&self.username.clone()?) || {
                match self.rooms.get(&msg.room) {
                    None => false,
                    Some(r) => r.direct,
                }
            };
            // not notifying the initial messages
            should_notify = should_notify && !init;
            // not notifying my own messages
            should_notify = should_notify && (msg.sender != self.uid.clone()?);

            if should_notify {
                self.notify(msg);
            }

            let command = InternalCommand::AddRoomMessage(msg.clone(), MsgPos::Bottom, prev, false,
                                                          self.is_first_new(&msg));
            self.internal.send(command).unwrap();
            prev = Some(msg.clone());

            if !init {
                self.roomlist.moveup(msg.room.clone());
                self.roomlist.set_bold(msg.room.clone(), true);
            }
        }

        if !msgs.is_empty() {
            let active_room = self.active_room.clone().unwrap_or_default();
            let fs = msgs.iter().filter(|x| x.room == active_room);
            if let Some(msg) = fs.last() {
                self.mark_as_read(msg, Force(false));
            }
        }

        if init {
            self.room_panel(RoomPanel::Room);
        }

        Some(())
    }

    pub fn show_room_messages_top(&mut self, msgs: Vec<Message>) {
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

            let prev = match i {
                n if size - n > 0 => msgs.get(size - n - 1).cloned(),
                _ => None
            };

            let command = InternalCommand::AddRoomMessage(msg.clone(), MsgPos::Top, prev, false,
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

        Some(create_ui_message(msg.clone(), name, t, highlights))
    }
}

/* FIXME: don't convert msg to ui messages here, we should later get a ui message from storage */
fn create_ui_message (msg: Message, name: Option<String>, t: RowType, highlights: Vec<String>) -> MessageContent {
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
        widget: None,
        }
}

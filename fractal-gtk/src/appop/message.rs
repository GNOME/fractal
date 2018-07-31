extern crate comrak;
extern crate tree_magic;

use i18n::i18n;

use std::path::Path;
use std::collections::HashMap;

use gtk;
use gtk::prelude::*;
use chrono::prelude::*;
use self::comrak::{markdown_to_html, ComrakOptions};

use app::InternalCommand;
use appop::AppOp;
use appop::RoomPanel;
use appop::room::Force;

use glib;
use globals;
use widgets;
use widgets::MessageContent;
use widgets::RoomHistory;
use widgets::RowType;
use backend::BKCommand;

use types::Message;

pub struct TmpMsg {
    pub msg: Message,
    pub widget: Option<gtk::Widget>,
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

    pub fn get_msg_from_id(&self, roomid: &str, msg_id: &str) -> Option<Message> {
        let room = self.rooms.get(roomid);

        room.and_then(|r| r.messages.clone().into_iter()
                                    .filter(|msg| msg.id.clone().unwrap_or_default() == msg_id)
                                    .next())
    }

    pub fn add_tmp_room_message(&mut self, msg: Message) {
        let messages = self.ui.builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        /* we have to track this also -> move to room_history
        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            let m;
            {
                let mb = widgets::MessageBox::new(&msg, &self.backend);
                m = mb.tmpwidget();
            }

            messages.add(&m);
        }
        */

        if let Some(w) = messages.get_children().iter().last() {
            self.msg_queue.insert(0, TmpMsg {
                    msg: msg.clone(),
                    widget: Some(w.clone()),
            });
        };
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
            /* we have to track this also -> move to room_history
            let mut widgets = vec![];
            for t in self.msg_queue.iter().rev().filter(|m| m.msg.room == r.id) {
                let m;
                {
                    let mb = widgets::MessageBox::new(&t.msg, &self.backend);
                    m = mb.tmpwidget();
                }

                messages.add(&m);
                if let Some(w) = messages.get_children().iter().last() {
                    widgets.push(w.clone());
                }
            }

            for (t, w) in self.msg_queue.iter_mut().rev().zip(widgets.iter()) {
                t.widget = Some(w.clone());
            }
            */
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
            receipt: HashMap::new(),
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
            receipt: HashMap::new(),
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
        let dialog = gtk::FileChooserDialog::new(None,
                                                 Some(&window),
                                                 gtk::FileChooserAction::Open);

        let btn = dialog.add_button(i18n("Select").as_str(), 1);
        btn.get_style_context().unwrap().add_class("suggested-action");

        let internal = self.internal.clone();
        dialog.connect_response(move |dialog, resp| {
            if resp == 1 {
                if let Some(fname) = dialog.get_filename() {
                    let f = String::from(fname.to_str().unwrap_or(""));
                    internal.send(InternalCommand::AttachMessage(f)).unwrap();
                }
            }
            dialog.destroy();
        });

        let internal = self.internal.clone();
        dialog.connect_file_activated(move |dialog| {
            if let Some(fname) = dialog.get_filename() {
                let f = String::from(fname.to_str().unwrap_or(""));
                internal.send(InternalCommand::AttachMessage(f)).unwrap();
            }
            dialog.destroy();
        });

        dialog.show();
    }

    pub fn load_more_messages(&mut self) {
        println!("We have to load more messages");
        if self.loading_more {
            return;
        }


        self.loading_more = true;
        self.load_more_spn.start();

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            if let Some(m) = r.messages.get(0) {
                self.backend.send(BKCommand::GetMessageContext(m.clone())).unwrap();
            } else {
                info!("The active room has no messages");
            }
        }
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

            /* add new message to room history only if they are from the active room */
            if msg.room == self.active_room.clone().unwrap_or_default() {
                let row = self.create_new_room_message(msg);
                if let Some(ref mut history) = self.room_history {
                    if let Some(row) = row {
                        history.add_new_message(row);
                    }
                }
            }

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

    /* parese a backend Message into a Message for the UI */
    pub fn create_new_room_message(&self, msg: &Message) -> Option<MessageContent> {
        /* set message type to mention if the body contains the username, we should
         * also match for MXID */
        let is_mention = if let Some(user) = self.username.clone() {
            msg.sender != self.uid.clone()? && msg.body.contains(&user)
        } else {
            false
        };

        let t = if msg.mtype == "m.emote" {
            RowType::Emote
        } else if is_mention {
            RowType::Mention
        } else {
            RowType::Message
        };

        let room = self.rooms.get(&msg.room)?;
        println!("Room: {:?}", room);
        let name = if let Some(member) = room.members.get(&msg.sender) {
            member.alias.clone()
        } else {
            None
        };

        Some(create_ui_message(msg.clone(), name, t))
    }

    pub fn show_room_messages_top(&mut self, msgs: Vec<Message>) {
        self.loading_more = false;
        /* FIXME: the context api returns the messages in the correct order, our backend does
         * revert the order for some reason */
        for msg in msgs.iter().rev() {
            if let Some(r) = self.rooms.get_mut(&msg.room) {
                r.messages.insert(0, msg.clone());
            }
        }

        if let Some(active_room) = self.active_room.clone() {
            for msg in msgs.iter().rev() {
                /* add old message to room history only if they are from the active room */
                if msg.room == active_room {
                    let row = self.create_new_room_message(msg);
                    if let Some(ref mut history) = self.room_history {
                        if let Some(row) = row {
                            history.add_old_message(row);
                        }
                    }
                }
            }
        }
    }
}

/* FIXME: don't convert msg to ui messages here */
fn create_ui_message (msg: Message, name: Option<String>, t: RowType) -> MessageContent {
    MessageContent {
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
    }
}

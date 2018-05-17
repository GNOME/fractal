extern crate gtk;
extern crate comrak;
extern crate chrono;
extern crate gettextrs;

use self::gtk::prelude::*;
use self::chrono::prelude::*;
use self::comrak::{markdown_to_html, ComrakOptions};
use self::gettextrs::gettext;

use app::InternalCommand;
use appop::AppOp;
use appop::RoomPanel;
use appop::room::Force;

use glib;
use globals;
use widgets;
use backend::BKCommand;

use types::Message;


#[derive(Debug, Clone)]
pub enum MsgPos {
    Top,
    Bottom,
}

pub struct TmpMsg {
    pub msg: Message,
    pub widget: gtk::Widget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LastViewed {
    Inline,
    Last,
    No,
}


impl AppOp {
    pub fn remove_messages(&mut self) {
        let messages = self.ui.builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");
        for ch in messages.get_children().iter().skip(1) {
            messages.remove(ch);
        }
    }

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

    fn should_group(&self, msg: &Message, prev: &Message) -> bool {
        let same_sender = msg.sender == prev.sender;

        match same_sender {
            true => {
                let diff = msg.date.signed_duration_since(prev.date);
                let minutes = diff.num_minutes();
                minutes < globals::MINUTES_TO_SPLIT_MSGS && !self.has_small_mtype(prev)
            },
            false => false,
        }
    }

    fn has_small_mtype(&self, msg: &Message) -> bool {
        match msg.mtype.as_ref() {
            "m.emote" => true,
            _ => false,
        }
    }

    pub fn is_last_viewed(&self, msg: &Message) -> LastViewed {
        match self.last_viewed_messages.get(&msg.room) {
            Some(lvm) if lvm == msg => {
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

    pub fn add_room_message(&mut self,
                            msg: Message,
                            msgpos: MsgPos,
                            prev: Option<Message>,
                            force_full: bool,
                            last: LastViewed) {
        let msg_entry: gtk::Entry = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");
        let messages = self.ui.builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        let mut calc_prev = prev;
        if !force_full && calc_prev.is_none() {
            if let Some(r) = self.rooms.get(&msg.room) {
                calc_prev = match r.messages.iter().position(|ref m| m.id == msg.id) {
                    Some(pos) if pos > 0 => r.messages.get(pos - 1).cloned(),
                    _ => None
                };
            }
        }

        if msg.room == self.active_room.clone().unwrap_or_default() {
            if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
                let m;
                {
                    let mb = widgets::MessageBox::new(r, &msg, &self);
                    let entry = msg_entry.clone();
                    mb.username_event_box.set_focus_on_click(false);
                    mb.username_event_box.connect_button_press_event(move |eb, _| {
                        if let Some(label) = eb.get_children().iter().nth(0) {
                            if let Ok(l) = label.clone().downcast::<gtk::Label>() {
                                if let Some(t) = l.get_text() {
                                    let mut pos = entry.get_position();
                                    entry.insert_text(&t[..], &mut pos);
                                    pos = entry.get_text_length() as i32;
                                    entry.set_position(pos);
                                    entry.grab_focus_without_selecting();
                                }
                            }
                        }
                        glib::signal::Inhibit(false)
                    });
                    m = match calc_prev {
                        Some(ref p) if self.should_group(&msg, p) => mb.small_widget(),
                        Some(_) if self.has_small_mtype(&msg) => mb.small_widget(),
                        _ => mb.widget(),
                    }
                }

                m.set_focus_on_click(false);

                match msgpos {
                    MsgPos::Bottom => messages.add(&m),
                    MsgPos::Top => messages.insert(&m, 1),
                };

                if last == LastViewed::Inline && msg.sender != self.uid.clone().unwrap_or_default() {
                    let divider: gtk::ListBoxRow = widgets::divider::new(gettext("New Messages").as_str());
                    match msgpos {
                        MsgPos::Bottom => messages.add(&divider),
                        MsgPos::Top => messages.insert(&divider, 2),
                    };
                }
                self.shown_messages += 1;
            }
            self.remove_tmp_room_message(&msg);
        }
    }

    pub fn add_tmp_room_message(&mut self, msg: Message) {
        let messages = self.ui.builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            let m;
            {
                let mb = widgets::MessageBox::new(r, &msg, &self);
                m = mb.widget();
            }

            messages.add(&m);
        }

        if let Some(w) = messages.get_children().iter().last() {
            self.tmp_msgs.push(TmpMsg {
                    msg: msg.clone(),
                    widget: w.clone(),
            });
        };
    }

    pub fn clear_tmp_msgs(&mut self) {
        let messages = self.ui.builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");
        for t in self.tmp_msgs.iter() {
            messages.remove(&t.widget);
        }
        self.tmp_msgs.clear();
    }

    pub fn remove_tmp_room_message(&mut self, msg: &Message) {
        let messages = self.ui.builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        let mut rmidx = None;

        for (i, t) in self.tmp_msgs.iter().enumerate() {
            if t.msg.sender == msg.sender &&
               t.msg.mtype == msg.mtype &&
               t.msg.room == msg.room &&
               t.msg.body == msg.body {

                messages.remove(&t.widget);
                rmidx = Some(i);
                break;
            }
        }

        if rmidx.is_some() {
            self.tmp_msgs.remove(rmidx.unwrap());
        }
    }

    pub fn mark_as_read(&mut self, msg: &Message, Force(force): Force) {
        let window: gtk::Window = self.ui.builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");
        if window.is_active() || force {
            self.last_viewed_messages.insert(msg.room.clone(), msg.clone());
            self.backend.send(BKCommand::MarkAsRead(msg.room.clone(),
                                                    msg.id.clone().unwrap_or_default())).unwrap();
        }
    }

    pub fn send_message(&mut self, msg: String) {
        if msg.is_empty() {
            // Not sending empty messages
            return;
        }

        let room = self.active_room.clone();
        let now = Local::now();

        let mtype = strn!("m.text");

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
        };

        if msg.starts_with("/me ") {
            m.body = msg.trim_left_matches("/me ").to_owned();
            m.mtype = strn!("m.emote");
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

        self.add_tmp_room_message(m.clone());
        self.backend.send(BKCommand::SendMsg(m)).unwrap();
    }

    pub fn attach_file(&mut self) {
        let window: gtk::ApplicationWindow = self.ui.builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");
        let dialog = gtk::FileChooserDialog::new(None,
                                                 Some(&window),
                                                 gtk::FileChooserAction::Open);

        let btn = dialog.add_button(gettext("Select").as_str(), 1);
        btn.get_style_context().unwrap().add_class("suggested-action");

        let backend = self.backend.clone();
        let room = self.active_room.clone().unwrap_or_default();
        dialog.connect_response(move |dialog, resp| {
            if resp == 1 {
                if let Some(fname) = dialog.get_filename() {
                    let f = strn!(fname.to_str().unwrap_or(""));
                    backend.send(BKCommand::AttachFile(room.clone(), f)).unwrap();
                }
            }
            dialog.destroy();
        });

        let backend = self.backend.clone();
        let room = self.active_room.clone().unwrap_or_default();
        dialog.connect_file_activated(move |dialog| {
            if let Some(fname) = dialog.get_filename() {
                let f = strn!(fname.to_str().unwrap_or(""));
                backend.send(BKCommand::AttachFile(room.clone(), f)).unwrap();
            }
            dialog.destroy();
        });

        dialog.show();
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
                                                                  self.is_last_viewed(&msg));
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

    pub fn show_room_messages(&mut self, msgs: Vec<Message>, init: bool) -> Option<()> {
        for msg in msgs.iter() {
            if let Some(r) = self.rooms.get_mut(&msg.room) {
                r.messages.push(msg.clone());
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
                                                          self.is_last_viewed(&msg));
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
                                                          self.is_last_viewed(&msg));
            self.internal.send(command).unwrap();

        }
        self.internal.send(InternalCommand::LoadMoreNormal).unwrap();
    }
}

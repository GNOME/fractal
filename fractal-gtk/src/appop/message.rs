use crate::backend::{room, HandleError};
use crate::types::ExtraContent;
use comrak::{markdown_to_html, ComrakOptions};
use fractal_api::identifiers::{EventId, RoomId};
use fractal_api::r0::AccessToken;
use fractal_api::url::Url;
use gdk_pixbuf::Pixbuf;
use gio::prelude::FileExt;
use glib::clone;
use glib::source::Continue;
use gtk::prelude::*;
use lazy_static::lazy_static;
use log::error;
use rand::Rng;
use serde_json::json;
use serde_json::Value as JsonValue;
use std::env::temp_dir;
use std::fs;
use std::path::PathBuf;
use std::thread;

use crate::appop::room::Force;
use crate::appop::AppOp;
use crate::App;

use crate::uitypes::MessageContent;
use crate::uitypes::RowType;
use crate::widgets;

use crate::types::Message;

pub struct TmpMsg {
    pub msg: Message,
    pub widget: Option<gtk::Widget>,
}

impl AppOp {
    pub fn get_message_by_id(&self, room_id: &RoomId, id: &EventId) -> Option<Message> {
        let room = self.rooms.get(room_id)?;
        let id = Some(id);
        room.messages.iter().find(|m| m.id.as_ref() == id).cloned()
    }

    /// This function is used to mark as read the last message of a room when the focus comes in,
    /// so we need to force the mark_as_read because the window isn't active yet
    pub fn mark_active_room_messages(&mut self) {
        self.mark_last_message_as_read(Force(true));
    }

    pub fn add_room_message(&mut self, msg: &Message) -> Option<()> {
        if let Some(ui_msg) = self.create_new_room_message(msg) {
            if let Some(ref mut history) = self.history {
                history.add_new_message(
                    self.thread_pool.clone(),
                    self.user_info_cache.clone(),
                    ui_msg,
                );
            }
        }
        None
    }

    pub fn remove_room_message(&mut self, msg: &Message) {
        if let Some(ui_msg) = self.create_new_room_message(msg) {
            if let Some(ref mut history) = self.history {
                history.remove_message(
                    self.thread_pool.clone(),
                    self.user_info_cache.clone(),
                    ui_msg,
                );
            }
        }
    }

    pub fn add_tmp_room_message(&mut self, msg: Message) -> Option<()> {
        let login_data = self.login_data.clone()?;
        let messages = self.history.as_ref()?.get_listbox();
        if let Some(ui_msg) = self.create_new_room_message(&msg) {
            let mb = widgets::MessageBox::new(login_data.server_url).tmpwidget(
                self.thread_pool.clone(),
                self.user_info_cache.clone(),
                &ui_msg,
            );
            let m = mb.get_listbox_row();
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
        let login_data = self.login_data.clone()?;
        let messages = self.history.as_ref()?.get_listbox();

        let r = self.rooms.get(self.active_room.as_ref()?)?;
        let mut widgets = vec![];
        for t in self.msg_queue.iter().rev().filter(|m| m.msg.room == r.id) {
            if let Some(ui_msg) = self.create_new_room_message(&t.msg) {
                let mb = widgets::MessageBox::new(login_data.server_url.clone()).tmpwidget(
                    self.thread_pool.clone(),
                    self.user_info_cache.clone(),
                    &ui_msg,
                );
                let m = mb.get_listbox_row();
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
        let login_data = self.login_data.clone()?;
        let window: gtk::Window = self
            .ui
            .builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");
        if window.is_active() || force {
            /* Move the last viewed mark to the last message */
            let active_room_id = self.active_room.as_ref()?;
            let room = self.rooms.get_mut(active_room_id)?;
            let uid = login_data.uid.clone();
            room.messages.iter_mut().for_each(|msg| {
                if msg.receipt.contains_key(&uid) {
                    msg.receipt.remove(&uid);
                }
            });
            let last_message = room.messages.last_mut()?;
            last_message.receipt.insert(uid, 0);

            let room_id = last_message.room.clone();
            let event_id = last_message.id.clone()?;
            thread::spawn(move || {
                match room::mark_as_read(
                    login_data.server_url,
                    login_data.access_token,
                    room_id,
                    event_id,
                ) {
                    Ok((r, _)) => {
                        APPOP!(clear_room_notifications, (r));
                    }
                    Err(err) => {
                        err.handle_error();
                    }
                }
            });
        }
        None
    }

    pub fn msg_sent(&mut self, _txid: String, evid: Option<EventId>) {
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
            Continue(false)
        });
    }

    pub fn force_dequeue_message(&mut self) {
        self.sending_message = false;
        self.dequeue_message();
    }

    pub fn dequeue_message(&mut self) -> Option<()> {
        let login_data = self.login_data.clone()?;
        if self.sending_message {
            return None;
        }

        self.sending_message = true;
        if let Some(next) = self.msg_queue.last() {
            let msg = next.msg.clone();
            match &next.msg.mtype[..] {
                "m.image" | "m.file" | "m.audio" | "m.video" => {
                    thread::spawn(move || {
                        attach_file(login_data.server_url, login_data.access_token, msg)
                    });
                }
                _ => {
                    thread::spawn(move || {
                        match room::send_msg(login_data.server_url, login_data.access_token, msg) {
                            Ok((txid, evid)) => {
                                APPOP!(msg_sent, (txid, evid));
                                let initial = false;
                                let number_tries = 0;
                                APPOP!(sync, (initial, number_tries));
                            }
                            Err(err) => {
                                err.handle_error();
                            }
                        }
                    });
                }
            }
        } else {
            self.sending_message = false;
        }
        None
    }

    pub fn send_message(&mut self, msg: String) {
        if msg.is_empty() {
            // Not sending empty messages
            return;
        }

        if let Some(room) = self.active_room.clone() {
            if let Some(sender) = self.login_data.as_ref().map(|ld| ld.uid.clone()) {
                let body = msg.clone();
                let mtype = String::from("m.text");
                let mut m = Message::new(room, sender, body, mtype, None);

                if msg.starts_with("/me ") {
                    m.body = msg.trim_start_matches("/me ").to_owned();
                    m.mtype = String::from("m.emote");
                }

                // Riot does not properly show emotes with Markdown;
                // Emotes with markdown have a newline after the username
                if m.mtype != "m.emote" && self.md_enabled {
                    let mut md_options = ComrakOptions::default();
                    md_options.hardbreaks = true;
                    let mut md_parsed_msg = markdown_to_html(&msg, &md_options);

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
            if let Some(sender) = self.login_data.as_ref().map(|ld| ld.uid.clone()) {
                if let Ok(info) = gio::File::new_for_path(&path).query_info(
                    &gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                    gio::FileQueryInfoFlags::NONE,
                    gio::NONE_CANCELLABLE,
                ) {
                    // This should always return a type
                    let mime = info
                        .get_content_type()
                        .expect("Could not parse content type from file");
                    let mtype = match mime.as_ref() {
                        m if m.starts_with("image") => "m.image",
                        m if m.starts_with("audio") => "m.audio",
                        "application/x-riff" => "m.audio",
                        m if m.starts_with("video") => "m.video",
                        "application/x-mpegURL" => "m.video",
                        _ => "m.file",
                    };
                    let body = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default();
                    let path_string = path.to_str().unwrap_or_default();

                    let mut m =
                        Message::new(room, sender, body.to_string(), mtype.to_string(), None);
                    let info = match mtype {
                        "m.image" => get_image_media_info(path_string, mime.as_ref()),
                        "m.audio" => get_audio_video_media_info(path_string, mime.as_ref()),
                        "m.video" => get_audio_video_media_info(path_string, mime.as_ref()),
                        "m.file" => get_file_media_info(path_string, mime.as_ref()),
                        _ => None,
                    };

                    m.extra_content = info;
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
            if let Some(w) = w.widget {
                w.destroy()
            }
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
        let login_data = self.login_data.clone()?;
        let uid = login_data.uid;
        for msg in msgs.iter() {
            if !msg.redacted && self.active_room.as_ref().map_or(false, |x| x == &msg.room) {
                self.add_room_message(&msg);
                msg_in_active = true;
            }

            if msg.replace != None {
                /* No need to notify (and confuse the user) about edits. */
                continue;
            }

            let should_notify = msg.sender != uid
                && (msg.body.contains(&login_data.username.clone()?)
                    || self.rooms.get(&msg.room).map_or(false, |r| r.direct));

            if should_notify {
                let window: gtk::Window = self
                    .ui
                    .builder
                    .get_object("main_window")
                    .expect("Can't find main_window in ui file.");
                if let (Some(app), Some(event_id)) = (window.get_application(), msg.id.as_ref()) {
                    self.notify(app, &msg.room, event_id);
                }
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
        room_id: RoomId,
        prev_batch: Option<String>,
    ) {
        if let Some(r) = self.rooms.get_mut(&room_id) {
            r.prev_batch = prev_batch;
        }

        let active_room = self.active_room.as_ref();
        let mut list = vec![];
        for item in msgs.iter().rev() {
            /* create a list of new messages to load to the history */
            if active_room.map_or(false, |a_room| item.room == *a_room) && !item.redacted {
                if let Some(ui_msg) = self.create_new_room_message(item) {
                    list.push(ui_msg);
                }
            }

            if let Some(r) = self.rooms.get_mut(&item.room) {
                r.messages.insert(0, item.clone());
            }
        }

        if let Some(ref mut history) = self.history {
            history.add_old_messages_in_batch(
                self.thread_pool.clone(),
                self.user_info_cache.clone(),
                list,
            );
        }
    }

    pub fn remove_message(&mut self, room_id: RoomId, id: EventId) -> Option<()> {
        let message = self.get_message_by_id(&room_id, &id);

        if let Some(msg) = message {
            self.remove_room_message(&msg);
            if let Some(ref mut room) = self.rooms.get_mut(&msg.room) {
                if let Some(ref mut message) = room.messages.iter_mut().find(|e| e.id == msg.id) {
                    message.redacted = true;
                }
            }
        }
        None
    }

    /* parese a backend Message into a Message for the UI */
    pub fn create_new_room_message(&self, msg: &Message) -> Option<MessageContent> {
        let login_data = self.login_data.clone()?;
        let mut highlights = vec![];
        lazy_static! {
            static ref EMOJI_REGEX: regex::Regex = regex::Regex::new(r"(?x)
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
                let is_mention = if let Some(user) = login_data.username.clone() {
                    msg.sender != login_data.uid && msg.body.contains(&user)
                } else {
                    false
                };

                if is_mention {
                    if let Some(user) = login_data.username {
                        highlights.push(user);
                    }
                    highlights.push(login_data.uid.to_string());
                    highlights.push(String::from("message_menu"));

                    RowType::Mention
                } else if EMOJI_REGEX.is_match(&msg.body) {
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

        let admin = room
            .admins
            .get(&login_data.uid)
            .copied()
            .unwrap_or_default();
        let redactable = admin != 0 || login_data.uid == msg.sender;

        let is_last_viewed = msg.receipt.contains_key(&login_data.uid);
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
        replace_date: if msg.replace.is_some() {
            Some(msg.date)
        } else {
            None
        },
        thumb: msg.thumb,
        url: msg.url,
        formatted_body: msg.formatted_body,
        format: msg.format,
        last_viewed,
        highlights,
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
    let mut rng = rand::thread_rng();
    let x: u64 = rng.gen_range(1, 9_223_372_036_854_775_807);
    let thumb_path = format!(
        "{}/fractal_{}.png",
        temp_dir().to_str().unwrap_or_default(),
        x.to_string()
    );
    thumb.savev(&thumb_path, "png", &[]).ok()?;
    let thumb_size = fs::metadata(&thumb_path).ok()?.len();

    let info = json!({
        "info": {
            "thumbnail_url": thumb_path,
            "thumbnail_info": {
                "w": thumb.get_width(),
                "h": thumb.get_height(),
                "size": thumb_size,
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

fn get_audio_video_media_info(file: &str, mimetype: &str) -> Option<JsonValue> {
    let size = fs::metadata(file).ok()?.len();

    if let Some(duration) = widgets::inline_player::get_media_duration(file)
        .ok()
        .and_then(|d| d.mseconds())
    {
        Some(json!({
            "info": {
                "size": size,
                "mimetype": mimetype,
                "duration": duration,
            }
        }))
    } else {
        Some(json!({
            "info": {
                "size": size,
                "mimetype": mimetype,
            }
        }))
    }
}

fn get_file_media_info(file: &str, mimetype: &str) -> Option<JsonValue> {
    let size = fs::metadata(file).ok()?.len();

    let info = json!({
        "info": {
            "size": size,
            "mimetype": mimetype,
        }
    });

    Some(info)
}

fn attach_file(baseu: Url, tk: AccessToken, mut msg: Message) {
    let fname = msg.url.clone().unwrap_or_default();
    let mut extra_content: Option<ExtraContent> = msg
        .clone()
        .extra_content
        .and_then(|c| serde_json::from_value(c).ok());

    let thumb = extra_content
        .clone()
        .and_then(|c| c.info.thumbnail_url)
        .unwrap_or_default();

    if fname.starts_with("mxc://") && thumb.starts_with("mxc://") {
        send_msg_and_manage(baseu, tk, msg);

        return;
    }

    if !thumb.is_empty() {
        match room::upload_file(baseu.clone(), tk.clone(), &thumb) {
            Ok(response) => {
                let thumb_uri = response.content_uri.to_string();
                msg.thumb = Some(thumb_uri.clone());
                if let Some(ref mut xctx) = extra_content {
                    xctx.info.thumbnail_url = Some(thumb_uri);
                }
                msg.extra_content = serde_json::to_value(&extra_content).ok();
            }
            Err(err) => {
                err.handle_error();
            }
        }

        if let Err(_e) = std::fs::remove_file(&thumb) {
            error!("Can't remove thumbnail: {}", thumb);
        }
    }

    let query = room::upload_file(baseu.clone(), tk.clone(), &fname).map(|response| {
        msg.url = Some(response.content_uri.to_string());
        thread::spawn(clone!(@strong msg => move || send_msg_and_manage(baseu, tk, msg)));

        msg
    });

    match query {
        Ok(msg) => {
            APPOP!(attached_file, (msg));
        }
        Err(err) => {
            err.handle_error();
        }
    };
}

fn send_msg_and_manage(baseu: Url, tk: AccessToken, msg: Message) {
    match room::send_msg(baseu, tk, msg) {
        Ok((txid, evid)) => {
            APPOP!(msg_sent, (txid, evid));
            let initial = false;
            let number_tries = 0;
            APPOP!(sync, (initial, number_tries));
        }
        Err(err) => {
            err.handle_error();
        }
    };
}

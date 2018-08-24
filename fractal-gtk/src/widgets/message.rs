extern crate gtk;
extern crate chrono;
extern crate pango;
extern crate glib;
extern crate regex;

use self::regex::Regex;
use itertools::Itertools;
use app::App;
use i18n::i18n;

use self::gtk::prelude::*;

use types::Message;
use types::Member;
use types::Room;

use self::chrono::prelude::*;

use backend::BKCommand;

use util::markup_text;

use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::TryRecvError;

use cache::download_to_cache;

use appop::AppOp;
use globals;
use widgets;
use widgets::AvatarExt;
use widgets::message_menu::MessageMenu;

// Room Message item
pub struct MessageBox<'a> {
    room: &'a Room,
    msg: &'a Message,
    op: &'a AppOp,
    username: gtk::Label,
    pub username_event_box: gtk::EventBox,
    pub row_event_box: gtk::EventBox,
}

impl<'a> MessageBox<'a> {
    pub fn new(room: &'a Room, msg: &'a Message, op: &'a AppOp) -> MessageBox<'a> {
        let username = gtk::Label::new("");
        let eb = gtk::EventBox::new();

        let row_eb = gtk::EventBox::new();
        let backend = op.backend.clone();
        let ui = op.ui.clone();

        row_eb.connect_button_press_event(clone!(msg => move |eb, btn| {
            if btn.get_button() == 3 {
                let menu = MessageMenu::new_message_menu(ui.clone(), backend.clone(),
                                                         msg.clone(), None);
                menu.show_menu_popover(eb.clone().upcast::<gtk::Widget>());
            }

            Inhibit(false)
        }));

        MessageBox {
            msg: msg,
            room: room,
            op: op,
            username: username,
            username_event_box: eb,
            row_event_box: row_eb,
        }
    }

    pub fn tmpwidget(&self) -> gtk::ListBoxRow {
        let w = self.widget();
        if let Some(style) = w.get_style_context() {
            style.add_class("msg-tmp");
        }
        w
    }

    pub fn widget(&self) -> gtk::ListBoxRow {
        // msg
        // +--------+---------+
        // | avatar | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 10);

        let content = self.build_room_msg_content(false);
        let avatar = self.build_room_msg_avatar();

        msg_widget.pack_start(&avatar, false, false, 0);
        msg_widget.pack_start(&content, true, true, 0);

        self.row_event_box.add(&msg_widget);

        let row = gtk::ListBoxRow::new();
        self.set_msg_styles(&row);
        row.set_selectable(false);
        row.set_margin_top(12);
        row.add(&self.row_event_box);
        row.show_all();

        row
    }

    pub fn small_widget(&self) -> gtk::ListBoxRow {
        // msg
        // +--------+---------+
        // |        | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 5);

        let content = self.build_room_msg_content(true);

        msg_widget.pack_start(&content, true, true, 50);

        self.row_event_box.add(&msg_widget);

        let row = gtk::ListBoxRow::new();
        self.set_msg_styles(&row);
        row.set_selectable(false);
        row.add(&self.row_event_box);
        row.show_all();

        row
    }

    fn build_room_msg_content(&self, small: bool) -> gtk::Box {
        // content
        // +------+
        // | info |
        // +------+
        // | body |
        // +------+
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let msg = self.msg;

        if !small {
            let info = self.build_room_msg_info(self.msg, small);
            info.set_margin_top(2);
            info.set_margin_bottom(3);
            content.pack_start(&info, false, false, 0);
        }

        let body = if !msg.redacted {
            match msg.mtype.as_ref() {
                "m.sticker" => self.build_room_msg_sticker(),
                "m.image" => self.build_room_msg_image(),
                "m.emote" => self.build_room_msg_emote(&msg),
                "m.audio" => self.build_room_audio_player(),
                "m.video" | "m.file" => self.build_room_msg_file(),
                _ => self.build_room_msg_body(&msg.body),
            }
        } else {
            self.build_room_msg_redacted()
        };

        content.pack_start(&body, true, true, 0);

        content
    }

    fn build_room_msg_avatar(&self) -> widgets::Avatar {
        let uid = self.msg.sender.clone();
        let avatar = widgets::Avatar::avatar_new(Some(globals::MSG_ICON_SIZE));

        let m = self.room.members.get(&uid);

        let data = match m {
            Some(member) => {
                self.username.set_text(&member.get_alias());
                let username = Some(member.get_alias());
                avatar.circle(uid.clone(), username, globals::MSG_ICON_SIZE)
            }
            None => {
                let backend = self.op.backend.clone();
                let data = avatar.circle(uid.clone(), None, globals::MSG_ICON_SIZE);
                let l = self.username.clone();
                let d = data.clone();
                set_username_async(backend, &uid, move |uname| {
                    l.set_text(&uname);
                    let mut data = d.borrow_mut();
                    data.redraw_fallback(Some(uname));
                });
                data
            }
        };

        download_to_cache(self.op.backend.clone(), uid.clone(),
                          data.clone());

        avatar
    }

    fn build_room_msg_username(&self, sender: &str, member: Option<&Member>, small: bool) -> gtk::Label {
        let uname = match member {
            Some(m) => m.get_alias(),
            None => {
                // in small widget, the avatar doesn't download the username
                // so we need to download here
                if small {
                    let backend = self.op.backend.clone();
                    let l = self.username.clone();
                    set_username_async(backend, sender, move |uname| {
                        l.set_text(&uname);
                    });
                }
                String::from(sender)
            }
        };

        self.username.set_text(&uname);
        self.username.set_justify(gtk::Justification::Left);
        self.username.set_halign(gtk::Align::Start);
        if let Some(style) = self.username.get_style_context() {
            style.add_class("username");
        }

        self.username.clone()
    }

    /// Add classes to the widget depending on the properties:
    ///
    ///  * msg-mention: if the message contains the username in the body and
    ///                 sender is not app user
    ///  * msg-emote: if the message is an emote
    fn set_msg_styles(&self, w: &gtk::ListBoxRow) {
        let uname = &self.op.username.clone().unwrap_or_default();
        let uid = self.op.uid.clone().unwrap_or_default();
        let msg = self.msg;
        let body: &str = &msg.body;

        if let Some(style) = w.get_style_context() {
            // mentions
            if String::from(body).contains(uname) && msg.sender != uid {
                style.add_class("msg-mention");
            }
            // emotes
            if msg.mtype == "m.emote" {
                style.add_class("msg-emote");
            }
        }
    }

    fn set_label_styles(&self, w: &gtk::Label) {
        w.set_line_wrap(true);
        w.set_line_wrap_mode(pango::WrapMode::WordChar);
        w.set_justify(gtk::Justification::Left);
        w.set_xalign(0.0);
        w.set_valign(gtk::Align::Start);
        w.set_halign(gtk::Align::Start);
        w.set_selectable(true);
    }

    fn build_room_msg_body(&self, body: &str) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let uname = self.op.username.clone().unwrap_or_default();

        let msg_parts = self.calculate_msg_parts(body);

        if self.msg.sender != self.op.uid.clone().unwrap_or_default()
            && String::from(body).contains(&uname) {
            for msg in msg_parts.iter() {
                let name = uname.clone();
                msg.connect_property_cursor_position_notify(move |w| {
                    if let Some(text) = w.get_text() {
                        if let Some(attr) = highlight_username(w.clone(), &name, text) {
                            w.set_attributes(&attr);
                        }
                    }
                });

                let name = uname.clone();
                msg.connect_property_selection_bound_notify(move |w| {
                    if let Some(text) = w.get_text() {
                        if let Some(attr) = highlight_username(w.clone(), &name, text) {
                            w.set_attributes(&attr);
                        }
                    }
                });

                if let Some(text) = msg.get_text() {
                    if let Some(attr) = highlight_username(msg.clone(), &uname, text) {
                        msg.set_attributes(&attr);
                    }
                }
            }
        }

        for part in msg_parts {
            bx.add(&part);
        }
        bx
    }

    fn build_room_msg_redacted(&self) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let redacted_label = gtk::Label::new("Message Deleted");
        if let Some(style) = redacted_label.get_style_context() {
            style.add_class("msg-redacted");
        }

        bx.add(&redacted_label);
        bx
    }

    fn calculate_msg_parts(&self, body: &str) -> Vec<gtk::Label> {
        let mut parts_labels: Vec<gtk::Label> = vec![];

        for (part, kind) in split_msg(body) {
            let msg_part = gtk::Label::new("");
            self.connect_right_click_menu(msg_part.clone().upcast::<gtk::Widget>());
            msg_part.set_markup(&markup_text(&part));
            self.set_label_styles(&msg_part);

            match kind {
                MsgPartType::Quote => {
                    msg_part.get_style_context().map(|s| s.add_class("quote"));
                }
                _ => {}
            }

            parts_labels.push(msg_part);
        }

        parts_labels
    }

    fn build_room_msg_image(&self) -> gtk::Box {
        let msg = self.msg;
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let backend = self.op.backend.clone();
        let img_path = match msg.thumb {
            Some(ref m) => m.clone(),
            None => msg.url.clone().unwrap_or_default(),
        };
        let image = widgets::image::Image::new(&backend, &img_path)
                        .size(Some(globals::MAX_IMAGE_SIZE)).build();

        let msg = msg.clone();
        let room_id = self.room.id.clone();
        image.widget.connect_button_press_event(move |_, btn| {
            if btn.get_button() != 3 {
                let msg = msg.clone();
                let rid = room_id.clone();
                APPOP!(display_media_viewer, (msg, rid));

                Inhibit(true)
            } else {
                Inhibit(false)
            }
        });

        if let Some(style) = image.widget.get_style_context() {
            style.add_class("image-widget");
        }

        bx.pack_start(&image.widget, true, true, 0);
        bx.show_all();
        bx
    }

    fn build_room_msg_sticker(&self) -> gtk::Box {
        let msg = self.msg;
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let backend = self.op.backend.clone();
        let image = widgets::image::Image::new(&backend,
                        &msg.url.clone().unwrap_or_default())
                        .size(Some(globals::MAX_STICKER_SIZE)).build();
        let w = image.widget.clone();
        w.set_tooltip_text(&self.msg.body[..]);

        bx.add(&w);

        bx
    }

    fn build_room_audio_player(&self) -> gtk::Box {
        let msg = self.msg;
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let player = widgets::AudioPlayerWidget::new();

        let name = msg.body.clone();
        let url = msg.url.clone().unwrap_or_default();
        let backend = self.op.backend.clone();

        let (tx, rx): (Sender<String>, Receiver<String>) = channel();
        backend.send(BKCommand::GetMediaUrl(url.clone(), tx)).unwrap();

        gtk::timeout_add(50, clone!(player => move || {
            match rx.try_recv() {
                Err(TryRecvError::Empty) => gtk::Continue(true),
                Err(TryRecvError::Disconnected) => {
                    let msg = i18n("Could not retrieve file URI");
                    APPOP!(show_error, (msg));
                    gtk::Continue(true)
                },
                Ok(uri) => {
                    println!("AUDIO URI: {}", &uri);
                    player.initialize_stream(&uri);
                    gtk::Continue(false)
                }
            }
        }));

        let download_btn = gtk::Button::new_from_icon_name(
            "document-save-symbolic",
            gtk::IconSize::Button.into(),
        );
        download_btn.set_tooltip_text(i18n("Save").as_str());

        download_btn.connect_clicked(clone!(name, url, backend => move |_| {
            let (tx, rx): (Sender<String>, Receiver<String>) = channel();

            backend.send(BKCommand::GetMediaAsync(url.clone(), tx)).unwrap();

            gtk::timeout_add(50, clone!(name => move || match rx.try_recv() {
                Err(TryRecvError::Empty) => gtk::Continue(true),
                Err(TryRecvError::Disconnected) => {
                    let msg = i18n("Could not download the file");
                    APPOP!(show_error, (msg));

                    gtk::Continue(true)
                },
                Ok(fname) => {
                    let name = name.clone();
                    APPOP!(save_file_as, (fname, name));

                    gtk::Continue(false)
                }
            }));
        }));

        bx.pack_start(&player.container, false, true, 0);
        bx.pack_start(&download_btn, false, false, 3);
        bx
    }

    fn build_room_msg_file(&self) -> gtk::Box {
        let msg = self.msg;
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let btn_bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let name = msg.body.clone();
        let url = msg.url.clone().unwrap_or_default();
        let backend = self.op.backend.clone();
        let name_lbl = gtk::Label::new(name.as_str());
        if let Some(style) = name_lbl.get_style_context() {
            style.add_class("msg-redacted");
        }

        let download_btn = gtk::Button::new_from_icon_name(
            "document-save-symbolic",
            gtk::IconSize::Button.into()
        );
        download_btn.set_tooltip_text(i18n("Save").as_str());

        download_btn.connect_clicked(clone!(name, url, backend => move |_| {
            let (tx, rx): (Sender<String>, Receiver<String>) = channel();


            backend.send(BKCommand::GetMediaAsync(url.clone(), tx)).unwrap();

            gtk::timeout_add(50, clone!(name => move || match rx.try_recv() {
                Err(TryRecvError::Empty) => gtk::Continue(true),
                Err(TryRecvError::Disconnected) => {
                    let msg = i18n("Could not download the file");
                    APPOP!(show_error, (msg));

                    gtk::Continue(true)
                },
                Ok(fname) => {
                    let name = name.clone();
                    APPOP!(save_file_as, (fname, name));

                    gtk::Continue(false)
                }
            }));
        }));

        let open_btn = gtk::Button::new_from_icon_name(
            "document-open-symbolic",
            gtk::IconSize::Button.into()
        );
        open_btn.set_tooltip_text(i18n("Open").as_str());

        open_btn.connect_clicked(clone!(url, backend => move |_| {
            backend.send(BKCommand::GetMedia(url.clone())).unwrap();
        }));

        btn_bx.pack_start(&open_btn, false, false, 0);
        btn_bx.pack_start(&download_btn, false, false, 0);
        if let Some(style) = btn_bx.get_style_context() {
            style.add_class("linked");
        }

        bx.pack_start(&name_lbl, false, false, 0);
        bx.pack_start(&btn_bx, false, false, 0);
        bx
    }

    fn build_room_msg_date(&self, dt: &DateTime<Local>) -> gtk::Label {
        let now = Local::now();

        let d = if (now.year() == dt.year()) && (now.ordinal() == dt.ordinal()) {
            dt.format("%H:%M").to_string()
        } else if now.year() == dt.year() {
            dt.format("%e %b %H:%M").to_string()
        } else {
            dt.format("%e %b %Y %H:%M").to_string()
        };

        let date = gtk::Label::new("");
        date.set_markup(&format!("<span alpha=\"60%\">{}</span>", d.trim()));
        date.set_line_wrap(true);
        date.set_justify(gtk::Justification::Right);
        date.set_valign(gtk::Align::Start);
        date.set_halign(gtk::Align::End);
        if let Some(style) = date.get_style_context() {
            style.add_class("timestamp");
        }

        date
    }

    fn build_room_msg_info(&self, msg: &Message, small: bool) -> gtk::Box {
        // info
        // +----------+------+
        // | username | date |
        // +----------+------+
        let info = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let member = self.room.members.get(&msg.sender);
        let username = self.build_room_msg_username(&msg.sender, member, small);
        let date = self.build_room_msg_date(&msg.date);

        self.username_event_box.add(&username);

        info.pack_start(&self.username_event_box, true, true, 0);
        info.pack_start(&date, false, false, 0);

        info
    }

    fn build_room_msg_emote(&self, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let member = self.room.members.get(&msg.sender);
        let sender: &str = &msg.sender;

        let msg_label = gtk::Label::new("");
        let body: &str = &msg.body;
        let markup = markup_text(body);

        let m = markup.clone();
        let sname = match member {
            Some(m) => m.get_alias(),
            None => {
                let backend = self.op.backend.clone();
                let label = msg_label.clone();
                set_username_async(backend, sender, move |n| {
                    label.set_markup(&format!("<b>{}</b> {}", n, m));
                });
                String::from(sender)
            }
        };

        self.connect_right_click_menu(msg_label.clone().upcast::<gtk::Widget>());
        msg_label.set_markup(&format!("<b>{}</b> {}", sname, markup));
        self.set_label_styles(&msg_label);

        bx.add(&msg_label);
        bx
    }

    fn connect_right_click_menu(&self, w: gtk::Widget) {
        let eb = self.row_event_box.clone();
        let msg = self.msg.clone();
        let backend = self.op.backend.clone();
        let ui = self.op.ui.clone();

        w.connect_button_press_event(move |w, btn| {
            if btn.get_button() == 3 {
                let menu = MessageMenu::new_message_menu(ui.clone(), backend.clone(),
                                                         msg.clone(), Some(w));
                menu.show_menu_popover(eb.clone().upcast::<gtk::Widget>());
                Inhibit(true)
            } else {
                Inhibit(false)
            }
        });
    }
}

fn highlight_username(label: gtk::Label, alias: &String, input: String) -> Option<pango::AttrList> {
    fn contains((start, end): (i32, i32), item: i32) -> bool {
        match start <= end {
            true => start <= item && end > item,
            false => start <= item || end > item,
        }
    }

    let input = input.to_lowercase();
    let bounds = label.get_selection_bounds();
    let context = gtk::Widget::get_style_context (&label.clone().upcast::<gtk::Widget>())?;
    let fg  = gtk::StyleContext::lookup_color (&context, "theme_selected_bg_color")?;
    let red = fg.red * 65535. + 0.5;
    let green = fg.green * 65535. + 0.5;
    let blue = fg.blue * 65535. + 0.5;
    let color = pango::Attribute::new_foreground(red as u16, green as u16, blue as u16)?;

    let attr = pango::AttrList::new();
    let mut input = input.clone();
    let alias = &alias.to_lowercase();
    let mut removed_char = 0;
    while input.contains(alias) {
        let pos = {
            let start = input.find(alias)? as i32;
            (start, start + alias.len() as i32)
        };
        let mut color = color.clone();
        let mark_start = removed_char as i32 + pos.0;
        let mark_end = removed_char as i32 + pos.1;
        let mut final_pos = Some((mark_start, mark_end));
        /* exclude selected text */
        if let Some((bounds_start, bounds_end)) = bounds {
            /* If the selection is within the alias */
            if contains((mark_start, mark_end), bounds_start) &&
                contains((mark_start, mark_end), bounds_end) {
                    final_pos = Some((mark_start, bounds_start));
                    /* Add blue color after a selection */
                    let mut color = color.clone();
                    color.set_start_index(bounds_end as u32);
                    color.set_end_index(mark_end as u32);
                    attr.insert(color);
                } else {
                    /* The alias starts inside a selection */
                    if contains(bounds?, mark_start) {
                        final_pos = Some((bounds_end, final_pos?.1));
                    }
                    /* The alias ends inside a selection */
                    if contains(bounds?, mark_end - 1) {
                        final_pos = Some((final_pos?.0, bounds_start));
                    }
                }
        }

        if let Some((start, end)) = final_pos {
            color.set_start_index(start as u32);
            color.set_end_index(end as u32);
            attr.insert(color);
        }
        {
            let end = pos.1 as usize;
            input.drain(0..end);
        }
        removed_char = removed_char + pos.1 as u32;
    }

    Some(attr)
}

fn set_username_async<F>(backend: Sender<BKCommand>,
                         uid: &str,
                         cb: F)
                         where F: Fn(String) + 'static {
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    backend.send(BKCommand::GetUserNameAsync(uid.to_string(), tx)).unwrap();
    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => gtk::Continue(false),
        Ok(username) => {
            cb(username);
            gtk::Continue(false)
        }
    });
}

#[derive(PartialEq)]
enum MsgPartType {
    Normal,
    Quote,
}

fn kind_of_line(line: &&str) -> MsgPartType {
    let r = Regex::new(r"^\s*> ?").unwrap();

    match line {
        l if r.is_match(l) => MsgPartType::Quote,
        _ => MsgPartType::Normal,
    }
}

fn trim_left_quote(line: &str) -> String {
    let r = Regex::new(r"^\s*> ?").unwrap();

    match r.is_match(line) {
        true => r.replace(line, "").to_string(),
        false => line.to_string(),
    }
}

fn trim_blank_lines(lines: String) -> String {
    let mut ret = lines;

    let r_start = Regex::new(r"^(\s*\n)+").unwrap();

    if r_start.is_match(&ret) {
        ret = r_start.replace(&ret, "").to_string();
    }

    ret.trim_right().to_string()
}

/// Split a message into parts depending on the kind
/// Currently supported types:
///  * Normal
///  * Quote
fn split_msg(body: &str) -> Vec<(String, MsgPartType)> {
    let mut parts: Vec<(String, MsgPartType)> = vec![];

    for (k, group) in body.lines()
                          .map(|l| l.trim_right())
                          .group_by(kind_of_line).into_iter() {
        let v: Vec<String> = group
            .map(|l| trim_left_quote(l))
            .collect();
        let s = trim_blank_lines(v.join("\n"));
        parts.push((s, k));
    }

    parts
}

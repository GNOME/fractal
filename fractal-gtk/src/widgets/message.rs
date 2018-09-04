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

use self::chrono::prelude::*;

use backend::BKCommand;

use util::markup_text;

use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::TryRecvError;

use cache::download_to_cache;
use cache::download_to_cache_username;

use globals;
use widgets;
use widgets::AvatarExt;
use widgets::message_menu::MessageMenu;
use widgets::RowType;
use widgets::MessageContent as Message;
use uibuilder::UI;

// Room Message item
pub struct MessageBox<'a> {
    msg: &'a Message,
    backend: Sender<BKCommand>,
    ui: &'a UI,
    username: gtk::Label,
    pub username_event_box: gtk::EventBox,
    pub row_event_box: gtk::EventBox,
    pub image: Option<gtk::DrawingArea>,
}

impl<'a> MessageBox<'a> {
    pub fn new(msg: &'a Message, backend: Sender<BKCommand>, ui: &'a UI) -> MessageBox<'a> {
        let username = gtk::Label::new("");
        let eb = gtk::EventBox::new();

        let row_eb = gtk::EventBox::new();
        let source_msg = msg.msg.clone();

        row_eb.connect_button_press_event(clone!(source_msg, backend, ui => move |eb, btn| {
            if btn.get_button() == 3 {
                let menu = MessageMenu::new_message_menu(ui.clone(), backend.clone(),
                                                         source_msg.clone(), None);
                menu.show_menu_popover(eb.clone().upcast::<gtk::Widget>());
            }

            Inhibit(false)
        }));

        MessageBox {
            msg: msg,
            backend: backend,
            ui: ui,
            username: username,
            username_event_box: eb,
            row_event_box: row_eb,
            image: None,
        }
    }

    pub fn tmpwidget(&mut self) -> gtk::ListBoxRow {
        let w = self.widget();
        if let Some(style) = w.get_style_context() {
            style.add_class("msg-tmp");
        }
        w
    }

    pub fn widget(&mut self) -> gtk::ListBoxRow {
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

    pub fn small_widget(&mut self) -> gtk::ListBoxRow {
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

    fn build_room_msg_content(&mut self, small: bool) -> gtk::Box {
        // content
        // +------+
        // | info |
        // +------+
        // | body |
        // +------+
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let msg = self.msg;

        if !small {
            let info = self.build_room_msg_info(self.msg);
            info.set_margin_top(2);
            info.set_margin_bottom(3);
            content.pack_start(&info, false, false, 0);
        }

        let body = match msg.mtype {
            RowType::Sticker => self.build_room_msg_sticker(),
            RowType::Image => self.build_room_msg_image(),
            RowType::Emote => self.build_room_msg_emote(&msg),
            RowType::Audio => self.build_room_audio_player(),
            RowType::Video | RowType::File => self.build_room_msg_file(),
            _ => self.build_room_msg_body(&msg.body),
        };

        content.pack_start(&body, true, true, 0);

        content
    }

    fn build_room_msg_avatar(&self) -> widgets::Avatar {
        let uid = self.msg.sender.clone();
        let alias = self.msg.sender_name.clone();
        let avatar = widgets::Avatar::avatar_new(Some(globals::MSG_ICON_SIZE));

        let data = avatar.circle(uid.clone(), alias.clone(), globals::MSG_ICON_SIZE);
        if let Some(name) = alias {
            self.username.set_text(&name);
        } else {
            self.username.set_text(&uid);
        }

        download_to_cache(self.backend.clone(), uid.clone(), data.clone());
        download_to_cache_username(self.backend.clone(), &uid, self.username.clone(), Some(data.clone()));

        avatar
    }

    fn build_room_msg_username(&self, sender: &str) -> gtk::Label {
        let uname = String::from(sender);

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
    ///  * msg-mention: if the message contains a keyword, e.g. the username
    ///  * msg-emote: if the message is an emote
    fn set_msg_styles(&self, w: &gtk::ListBoxRow) {
        if let Some(style) = w.get_style_context() {
            match self.msg.mtype {
                RowType::Mention => style.add_class("msg-mention"),
                RowType::Emote => style.add_class("msg-emote"),
                _ => {},
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

        let msg_parts = self.calculate_msg_parts(body);

        if self.msg.mtype == RowType::Mention {
            for msg in msg_parts.iter() {
                let highlights = self.msg.highlights.clone();
                msg.connect_property_cursor_position_notify(move |w| {
                if let Some(text) = w.get_text() {
                    let attr = pango::AttrList::new();
                    for light in highlights.clone() {
                        highlight_username(w.clone(), &attr, &light, text.clone());
                    }
                    w.set_attributes(&attr);
                }
                });

                let highlights = self.msg.highlights.clone();
                msg.connect_property_selection_bound_notify(move |w| {
                if let Some(text) = w.get_text() {
                    let attr = pango::AttrList::new();
                    for light in highlights.clone() {
                        highlight_username(w.clone(), &attr, &light, text.clone());
                    }
                    w.set_attributes(&attr);
                }
                });

                if let Some(text) = msg.get_text() {
                    let attr = pango::AttrList::new();
                    for light in self.msg.highlights.clone() {
                        highlight_username(msg.clone(), &attr, &light, text.clone());
                    }
                    msg.set_attributes(&attr);
                }
            }
        }

        for part in msg_parts {
            bx.add(&part);
        }
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

    fn build_room_msg_image(&mut self) -> gtk::Box {
        let msg = self.msg;
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let img_path = match msg.thumb {
            Some(ref m) => m.clone(),
            None => msg.url.clone().unwrap_or_default(),
        };
        let image = widgets::image::Image::new(&self.backend, &img_path)
                        .size(Some(globals::MAX_IMAGE_SIZE)).build();
        if let Some(style) = image.widget.get_style_context() {
            style.add_class("image-widget");
        }

        bx.pack_start(&image.widget, true, true, 0);
        bx.show_all();
        self.image = Some(image.widget);
        bx
    }

    fn build_room_msg_sticker(&self) -> gtk::Box {
        let msg = self.msg;
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let backend = self.backend.clone();
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
        let backend = self.backend.clone();

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
        let backend = self.backend.clone();
        let name_lbl = gtk::Label::new(name.as_str());
        name_lbl.set_tooltip_text(name.as_str());
        name_lbl.set_ellipsize(pango::EllipsizeMode::End);

        if let Some(style) = name_lbl.get_style_context() {
            style.add_class("msg-highlighted");
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

    fn build_room_msg_info(&self, msg: &Message) -> gtk::Box {
        // info
        // +----------+------+
        // | username | date |
        // +----------+------+
        let info = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let username = self.build_room_msg_username(&msg.sender);
        let date = self.build_room_msg_date(&msg.date);

        self.username_event_box.add(&username);

        info.pack_start(&self.username_event_box, true, true, 0);
        info.pack_start(&date, false, false, 0);

        info
    }

    fn build_room_msg_emote(&self, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        /* Use MXID till we have a alias */
        let sname = msg.sender_name.clone().unwrap_or(String::from(msg.sender.clone()));
        let msg_label = gtk::Label::new("");
        let body: &str = &msg.body;
        let markup = markup_text(body);

        /*
        let m = markup.clone();
        let sname = match member {
            Some(m) => m.get_alias(),
            None => {
                let backend = self.backend.clone();
                let label = msg_label.clone();
                set_username_async(backend, sender, move |n| {
                    label.set_markup(&format!("<b>{}</b> {}", n, m));
                });
                String::from(sender)
            }
        };
        */

        self.connect_right_click_menu(msg_label.clone().upcast::<gtk::Widget>());
        msg_label.set_markup(&format!("<b>{}</b> {}", sname, markup));
        self.set_label_styles(&msg_label);

        bx.add(&msg_label);
        bx
    }

    fn connect_right_click_menu(&self, w: gtk::Widget) {
        let eb = self.row_event_box.clone();
        let backend = self.backend.clone();
        let ui = self.ui.clone();
        let msg = self.msg.msg.clone();

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

fn highlight_username(label: gtk::Label, attr: &pango::AttrList, alias: &String, input: String) -> Option<()> {
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

    None
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

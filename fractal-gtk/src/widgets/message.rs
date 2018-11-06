use itertools::Itertools;
use app::App;
use i18n::i18n;

use pango;
use glib;
use gtk;
use gtk::prelude::*;
use chrono::prelude::*;

use backend::BKCommand;

use util::markup_text;

use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::TryRecvError;

use cache::download_to_cache;
use cache::download_to_cache_username;
use cache::download_to_cache_username_emote;

use globals;
use widgets;
use widgets::AvatarExt;
use widgets::message_menu::MessageMenu;
use uitypes::RowType;
use uitypes::MessageContent as Message;
use uibuilder::UI;

/* A message row in the room history */
#[derive(Clone)]
pub struct MessageBox {
    msg: Message,
    backend: Sender<BKCommand>,
    /* FIXME: Remove UI */
    ui: UI,
    username: gtk::Label,
    pub username_event_box: gtk::EventBox,
    widget: gtk::EventBox,
    row: Option<gtk::ListBoxRow>,
    pub image: Option<gtk::DrawingArea>,
    header: bool,
}

impl MessageBox {
    pub fn new(msg: Message, backend: Sender<BKCommand>, ui: UI) -> MessageBox {
        let username = gtk::Label::new("");
        let eb = gtk::EventBox::new();

        let row_eb = gtk::EventBox::new();

        row_eb.connect_button_press_event(clone!(msg, backend, ui => move |eb, btn| {
            if btn.get_button() == 3 {
                let menu = MessageMenu::new_message_menu(ui.clone(), backend.clone(),
                                                         msg.clone(), None);
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
            widget: row_eb,
            row: None,
            image: None,
            header: true,
        }
    }

    /* create the message row with or without a header */
    pub fn create(&mut self, has_header: bool) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        self.set_msg_styles(&row);
        row.set_selectable(false);
        let w = if has_header && self.msg.mtype != RowType::Emote {
            row.set_margin_top(12);
            self.header = true;
            self.widget()
        } else {
            self.header = false;
            self.small_widget()
        };

        self.widget.add(&w);
        row.add(&self.widget);
        row.show_all();
        self.row = Some(row.clone());
        row
    }

    /* Updates the header of a message row */
    #[allow(dead_code)]
    pub fn update(&mut self, has_header: bool) -> Option<()> {
        /* Update only if some thing changed */
        if has_header != self.header {
            self.username.destroy();
            self.username = gtk::Label::new("");
            self.username_event_box.destroy();
            self.username_event_box = gtk::EventBox::new();
            let row = self.row.clone()?;
            let child = self.widget.get_child()?;
            self.widget.remove(&child);
            let w = if has_header && self.msg.mtype != RowType::Emote {
                row.set_margin_top(12);
                self.header = true;
                self.widget()
            } else {
                /* we need to reset the margin */
                row.set_margin_top(0);
                self.header = false;
                self.small_widget()
            };

            self.widget.add(&w);
            row.show_all();
        }
        None
    }

    pub fn tmpwidget(&mut self) -> gtk::ListBoxRow {
        let w = self.create(true);
        if let Some(style) = w.get_style_context() {
            style.add_class("msg-tmp");
        }
        w
    }

    fn widget(&mut self) -> gtk::Box {
        // msg
        // +--------+---------+
        // | avatar | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let content = self.build_room_msg_content(false);
        /* Todo: make build_room_msg_avatar() faster (currently ~1ms) */
        let avatar = self.build_room_msg_avatar();

        msg_widget.pack_start(&avatar, false, false, 0);
        msg_widget.pack_start(&content, true, true, 0);

        msg_widget
    }

    fn small_widget(&mut self) -> gtk::Box {
        // msg
        // +--------+---------+
        // |        | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        let content = self.build_room_msg_content(true);
        msg_widget.pack_start(&content, true, true, 50);

        msg_widget
    }

    fn build_room_msg_content(&mut self, small: bool) -> gtk::Box {
        // content
        // +------+
        // | info |
        // +------+
        // | body |
        // +------+
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);

        if !small {
            let info = self.build_room_msg_info(&self.msg);
            info.set_margin_top(2);
            info.set_margin_bottom(3);
            content.pack_start(&info, false, false, 0);
        }

        let body = match self.msg.mtype {
            RowType::Sticker => self.build_room_msg_sticker(),
            RowType::Image => self.build_room_msg_image(),
            RowType::Emote => self.build_room_msg_emote(&self.msg),
            RowType::Audio => self.build_room_audio_player(),
            RowType::Video | RowType::File => self.build_room_msg_file(),
            _ => self.build_room_msg_body(&self.msg.body),
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

    /* Add classes to the widget based on message type */
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

        let msg_parts = self.create_msg_parts(body);

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

    fn create_msg_parts(&self, body: &str) -> Vec<gtk::Label> {
        let mut parts_labels: Vec<gtk::Label> = vec![];

        for (k, group) in body.lines().group_by(kind_of_line).into_iter() {
            let mut v: Vec<&str> = if k == MsgPartType::Quote {
                group.map(|l| trim_start_quote(l)).collect()
            } else {
                group.collect()
            };
            /* We need to remove the first and last empty line (if any) because quotes use /n/n */
            if v.starts_with(&[""]) {
                v.drain(..1);
            }
            if v.ends_with(&[""]) {
                v.pop();
            }
            let part = v.join("\n");

            parts_labels.push(self.create_msg(part.as_str(), k));
        }

        parts_labels
    }

    fn create_msg(&self, body: &str, k: MsgPartType) -> gtk::Label {
        let msg_part = gtk::Label::new("");
        self.connect_right_click_menu(msg_part.clone().upcast::<gtk::Widget>());
        msg_part.set_markup(&markup_text(&body));
        self.set_label_styles(&msg_part);

        if k == MsgPartType::Quote {
            msg_part.get_style_context().map(|s| s.add_class("quote"));
        }
        msg_part
    }

    fn build_room_msg_image(&mut self) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let img_path = match self.msg.thumb {
            Some(ref m) => m.clone(),
            None => self.msg.url.clone().unwrap_or_default(),
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
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let backend = self.backend.clone();
        let image = widgets::image::Image::new(&backend,
                        &self.msg.url.clone().unwrap_or_default())
                        .size(Some(globals::MAX_STICKER_SIZE)).build();
        let w = image.widget.clone();
        w.set_tooltip_text(&self.msg.body[..]);

        bx.add(&w);

        bx
    }

    fn build_room_audio_player(&self) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let player = widgets::AudioPlayerWidget::new();

        let name = self.msg.body.clone();
        let url = self.msg.url.clone().unwrap_or_default();
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
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let btn_bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let name = self.msg.body.clone();
        let url = self.msg.url.clone().unwrap_or_default();
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

        download_to_cache_username_emote(self.backend.clone(), &sname, &markup, msg_label.clone(), None);

        self.connect_right_click_menu(msg_label.clone().upcast::<gtk::Widget>());
        msg_label.set_markup(&format!("<b>{}</b> {}", sname, markup));
        self.set_label_styles(&msg_label);

        bx.add(&msg_label);
        bx
    }

    fn connect_right_click_menu(&self, w: gtk::Widget) {
        let eb = self.widget.clone();
        let backend = self.backend.clone();
        let ui = self.ui.clone();
        let msg = self.msg.clone();

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
    if line.trim_start().starts_with(">") {
        MsgPartType::Quote
    } else {
        MsgPartType::Normal
    }
}

fn trim_start_quote(line: &str) -> &str {
    line.trim_start().get(1..).unwrap_or(line).trim_start()
}

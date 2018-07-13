extern crate gtk;
extern crate chrono;
extern crate pango;
extern crate glib;

use app::App;
use i18n::i18n;

use std::cell::RefCell;
use std::rc::Rc;

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
use widgets::AvatarData;

// Room Message item
pub struct MessageBox<'a> {
    room: &'a Room,
    msg: &'a Message,
    op: &'a AppOp,
    username: gtk::Label,
    pub username_event_box: gtk::EventBox,
}

impl<'a> MessageBox<'a> {
    pub fn new(room: &'a Room, msg: &'a Message, op: &'a AppOp) -> MessageBox<'a> {
        let username = gtk::Label::new("");
        let eb = gtk::EventBox::new();

        MessageBox {
            msg: msg,
            room: room,
            op: op,
            username: username,
            username_event_box: eb,
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

        let row = gtk::ListBoxRow::new();
        self.set_msg_styles(&row);
        row.set_selectable(false);
        row.set_margin_top(12);
        row.add(&msg_widget);
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

        let row = gtk::ListBoxRow::new();
        self.set_msg_styles(&row);
        row.set_selectable(false);
        row.add(&msg_widget);
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

        let body = match msg.mtype.as_ref() {
            "m.sticker" => self.build_room_msg_sticker(),
            "m.image" => self.build_room_msg_image(),
            "m.emote" => self.build_room_msg_emote(&msg),
            "m.audio" => self.build_room_audio_player(),
            "m.video" | "m.file" => self.build_room_msg_file(),
            _ => self.build_room_msg_body(&msg.body),
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
                set_username_async(backend, &uid, self.username.clone(),
                                   Some(data.clone()));
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
                    set_username_async(backend, sender, self.username.clone(), None);
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
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let msg = gtk::Label::new("");
        let uname = self.op.username.clone().unwrap_or_default();

        msg.set_markup(&markup_text(body));
        self.set_label_styles(&msg);

        if self.msg.sender != self.op.uid.clone().unwrap_or_default()
            && String::from(body).contains(&uname) {

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

        bx.add(&msg);
        bx
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
        image.widget.connect_button_press_event(move |_, _| {
            let msg = msg.clone();
            let rid = room_id.clone();
            APPOP!(display_media_viewer, (msg, rid));

            Inhibit(true)
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
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let viewbtn = gtk::Button::new();
        let name = msg.body.clone();
        let url = msg.url.clone().unwrap_or_default();
        let backend = self.op.backend.clone();
        viewbtn.connect_clicked(move |btn| {
            let popover = gtk::Popover::new(btn);

            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

            let download_btn = gtk::ModelButton::new();
            download_btn.set_label(&i18n("Download"));

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

            vbox.pack_start(&download_btn, false, false, 6);

            let open_btn = gtk::ModelButton::new();
            open_btn.set_label(&i18n("Open"));

            open_btn.connect_clicked(clone!(url, backend => move |_| {
                backend.send(BKCommand::GetMedia(url.clone())).unwrap();
            }));

            vbox.pack_start(&open_btn, false, false, 6);

            vbox.show_all();
            popover.add(&vbox);
            popover.popup();
        });

        viewbtn.set_label(&msg.body);

        bx.add(&viewbtn);
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

        let sname = match member {
            Some(m) => m.get_alias(),
            None => String::from(sender),
        };

        let msg_label = gtk::Label::new("");
        let body: &str = &msg.body;

        msg_label.set_markup(&format!("<b>{}</b> {}", sname, markup_text(body)));

        self.set_label_styles(&msg_label);

        bx.add(&msg_label);
        bx
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

fn set_username_async(backend: Sender<BKCommand>,
                      uid: &str,
                      label: gtk::Label,
                      avatar: Option<Rc<RefCell<AvatarData>>>) {
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    backend.send(BKCommand::GetUserNameAsync(uid.to_string(), tx)).unwrap();
    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => gtk::Continue(false),
        Ok(username) => {
            label.set_text(&username);
            if let Some(ref rc_data) = avatar {
                let mut data = rc_data.borrow_mut();
                data.redraw_fallback(Some(username));
            }

            gtk::Continue(false)
        }
    });
}

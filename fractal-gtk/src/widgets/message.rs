use crate::app::App;
use crate::i18n::i18n;
use fractal_api::clone;
use itertools::Itertools;
use log::info;

use chrono::prelude::*;
use glib;
use gtk;
use gtk::prelude::*;
use pango;

use crate::backend::BKCommand;

use crate::util::markup_text;

use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};

use crate::cache::download_to_cache;
use crate::cache::download_to_cache_username;
use crate::cache::download_to_cache_username_emote;

use crate::globals;
use crate::uitypes::MessageContent as Message;
use crate::uitypes::RowType;
use crate::widgets;
use crate::widgets::message_menu::MessageMenu;
use crate::widgets::AvatarExt;

/* A message row in the room history */
#[derive(Clone, Debug)]
pub struct MessageBox {
    backend: Sender<BKCommand>,
    username: gtk::Label,
    pub username_event_box: gtk::EventBox,
    eventbox: gtk::EventBox,
    gesture: gtk::GestureLongPress,
    row: gtk::ListBoxRow,
    image: Option<gtk::DrawingArea>,
    header: bool,
}

impl MessageBox {
    pub fn new(backend: Sender<BKCommand>) -> MessageBox {
        let username = gtk::Label::new("");
        let eb = gtk::EventBox::new();
        let eventbox = gtk::EventBox::new();
        let row = gtk::ListBoxRow::new();
        let gesture = gtk::GestureLongPress::new(&eventbox);

        username.set_ellipsize(pango::EllipsizeMode::End);
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        gesture.set_touch_only(true);

        MessageBox {
            backend: backend,
            username: username,
            username_event_box: eb,
            eventbox,
            gesture,
            row,
            image: None,
            header: true,
        }
    }

    /* create the message row with or without a header */
    pub fn create(&mut self, msg: &Message, has_header: bool) {
        self.set_msg_styles(msg, &self.row);
        self.row.set_selectable(false);
        let w = if has_header && msg.mtype != RowType::Emote {
            self.row.set_margin_top(12);
            self.header = true;
            self.widget(msg)
        } else {
            if let RowType::Emote = msg.mtype {
                self.row.set_margin_top(12);
            }
            self.header = false;
            self.small_widget(msg)
        };

        self.eventbox.add(&w);
        self.row.add(&self.eventbox);
        self.row.show_all();
        self.connect_right_click_menu(msg, None);
    }

    // FIXME: return directly row
    pub fn get_listbox_row(&self) -> Option<&gtk::ListBoxRow> {
        Some(&self.row)
    }

    pub fn tmpwidget(mut self, msg: &Message) -> Option<MessageBox> {
        self.create(msg, true);
        {
            let w = self.get_listbox_row()?;
            let style = w.get_style_context()?;
            style.add_class("msg-tmp");
        }
        Some(self)
    }

    fn widget(&mut self, msg: &Message) -> gtk::Box {
        // msg
        // +--------+---------+
        // | avatar | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let content = self.build_room_msg_content(msg, false);
        /* Todo: make build_room_msg_avatar() faster (currently ~1ms) */
        let avatar = self.build_room_msg_avatar(msg);

        msg_widget.pack_start(&avatar, false, false, 0);
        msg_widget.pack_start(&content, true, true, 0);

        msg_widget
    }

    fn small_widget(&mut self, msg: &Message) -> gtk::Box {
        // msg
        // +--------+---------+
        // |        | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        let content = self.build_room_msg_content(msg, true);
        msg_widget.pack_start(&content, true, true, 50);

        msg_widget
    }

    fn build_room_msg_content(&mut self, msg: &Message, small: bool) -> gtk::Box {
        // content
        // +------+
        // | info |
        // +------+
        // | body |
        // +------+
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);

        if !small {
            let info = self.build_room_msg_info(msg);
            info.set_margin_top(2);
            info.set_margin_bottom(3);
            content.pack_start(&info, false, false, 0);
        }

        let body = match msg.mtype {
            RowType::Sticker => self.build_room_msg_sticker(msg),
            RowType::Image => self.build_room_msg_image(msg),
            RowType::Emote => self.build_room_msg_emote(msg),
            RowType::Audio => self.build_room_audio_player(msg),
            RowType::Video | RowType::File => self.build_room_msg_file(msg),
            _ => self.build_room_msg_body(msg),
        };

        content.pack_start(&body, true, true, 0);

        content
    }

    fn build_room_msg_avatar(&self, msg: &Message) -> widgets::Avatar {
        let uid = msg.sender.clone();
        let alias = msg.sender_name.clone();
        let avatar = widgets::Avatar::avatar_new(Some(globals::MSG_ICON_SIZE));

        let data = avatar.circle(uid.clone(), alias.clone(), globals::MSG_ICON_SIZE);
        if let Some(name) = alias {
            self.username.set_text(&name);
        } else {
            self.username.set_text(&uid);
        }

        download_to_cache(self.backend.clone(), uid.clone(), data.clone());
        download_to_cache_username(
            self.backend.clone(),
            &uid,
            self.username.clone(),
            Some(data.clone()),
        );

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
    fn set_msg_styles(&self, msg: &Message, w: &gtk::ListBoxRow) {
        if let Some(style) = w.get_style_context() {
            match msg.mtype {
                RowType::Mention => style.add_class("msg-mention"),
                RowType::Emote => style.add_class("msg-emote"),
                RowType::Emoji => style.add_class("msg-emoji"),
                _ => {}
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

    fn build_room_msg_body(&self, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Vertical, 6);

        let msg_parts = self.create_msg_parts(&msg.body);

        if msg.mtype == RowType::Mention {
            for part in msg_parts.iter() {
                let highlights = msg.highlights.clone();
                part.connect_property_cursor_position_notify(move |w| {
                    if let Some(text) = w.get_text() {
                        let attr = pango::AttrList::new();
                        for light in highlights.clone() {
                            highlight_username(w.clone(), &attr, &light, text.clone());
                        }
                        w.set_attributes(&attr);
                    }
                });

                let highlights = msg.highlights.clone();
                part.connect_property_selection_bound_notify(move |w| {
                    if let Some(text) = w.get_text() {
                        let attr = pango::AttrList::new();
                        for light in highlights.clone() {
                            highlight_username(w.clone(), &attr, &light, text.clone());
                        }
                        w.set_attributes(&attr);
                    }
                });

                if let Some(text) = part.get_text() {
                    let attr = pango::AttrList::new();
                    for light in msg.highlights.clone() {
                        highlight_username(part.clone(), &attr, &light, text.clone());
                    }
                    part.set_attributes(&attr);
                }
            }
        }

        for part in msg_parts {
            self.connect_right_click_menu(msg, Some(&part));
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
        msg_part.set_markup(&markup_text(body));
        self.set_label_styles(&msg_part);

        if k == MsgPartType::Quote {
            msg_part.get_style_context().map(|s| s.add_class("quote"));
        }
        msg_part
    }

    fn build_room_msg_image(&mut self, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let img_path = match msg.thumb {
            Some(ref m) => m.clone(),
            None => msg.url.clone().unwrap_or_default(),
        };
        let image = widgets::image::Image::new(&self.backend, &img_path)
            .size(Some(globals::MAX_IMAGE_SIZE))
            .build();

        if let Some(style) = image.widget.get_style_context() {
            style.add_class("image-widget");
        }

        bx.pack_start(&image.widget, true, true, 0);
        bx.show_all();
        self.image = Some(image.widget);
        self.connect_image(msg);

        bx
    }

    fn build_room_msg_sticker(&self, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let backend = self.backend.clone();
        if let Some(url) = msg.url.as_ref() {
            let image = widgets::image::Image::new(&backend, url)
                .size(Some(globals::MAX_STICKER_SIZE))
                .build();
            image.widget.set_tooltip_text(&msg.body[..]);

            bx.add(&image.widget);
        }

        bx
    }

    fn build_room_audio_player(&self, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let player = widgets::AudioPlayerWidget::new();

        let url = msg.url.clone().unwrap_or_default();
        let backend = self.backend.clone();

        let (tx, rx): (Sender<String>, Receiver<String>) = channel();
        backend
            .send(BKCommand::GetMediaUrl(url.clone(), tx))
            .unwrap();

        gtk::timeout_add(
            50,
            clone!(player => move || {
                match rx.try_recv() {
                    Err(TryRecvError::Empty) => gtk::Continue(true),
                    Err(TryRecvError::Disconnected) => {
                        let msg = i18n("Could not retrieve file URI");
                        /* FIXME: don't use APPOP! */
                        APPOP!(show_error, (msg));
                        gtk::Continue(true)
                    },
                    Ok(uri) => {
                        info!("AUDIO URI: {}", &uri);
                        player.initialize_stream(&uri);
                        gtk::Continue(false)
                    }
                }
            }),
        );

        let download_btn =
            gtk::Button::new_from_icon_name("document-save-symbolic", gtk::IconSize::Button.into());
        download_btn.set_tooltip_text(i18n("Save").as_str());

        let data = glib::Variant::from(msg.id.as_str());
        download_btn.set_action_target_value(&data);
        download_btn.set_action_name("room_history.save_as");

        bx.pack_start(&player.container, false, true, 0);
        bx.pack_start(&download_btn, false, false, 3);
        bx
    }

    fn build_room_msg_file(&self, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let btn_bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let name = msg.body.as_str();
        let name_lbl = gtk::Label::new(name);
        name_lbl.set_tooltip_text(name);
        name_lbl.set_ellipsize(pango::EllipsizeMode::End);

        if let Some(style) = name_lbl.get_style_context() {
            style.add_class("msg-highlighted");
        }

        let download_btn =
            gtk::Button::new_from_icon_name("document-save-symbolic", gtk::IconSize::Button.into());
        download_btn.set_tooltip_text(i18n("Save").as_str());

        let data = glib::Variant::from(msg.id.as_str());
        download_btn.set_action_target_value(&data);
        download_btn.set_action_name("room_history.save_as");

        let open_btn =
            gtk::Button::new_from_icon_name("document-open-symbolic", gtk::IconSize::Button.into());
        open_btn.set_tooltip_text(i18n("Open").as_str());

        let data = glib::Variant::from(msg.id.as_str());
        open_btn.set_action_target_value(&data);
        open_btn.set_action_name("room_history.open_with");

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
        /* TODO: get system preference for 12h/24h */
        let use_ampm = false;
        let format = if use_ampm {
            /* Use 12h time format (AM/PM) */
            i18n("%l∶%M %p")
        } else {
            /* Use 24 time format */
            i18n("%R")
        };

        let d = dt.format(&format).to_string();

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
        let sname = msg
            .sender_name
            .clone()
            .unwrap_or(String::from(msg.sender.clone()));
        let msg_label = gtk::Label::new("");
        let body: &str = &msg.body;
        let markup = markup_text(body);

        download_to_cache_username_emote(
            self.backend.clone(),
            &sname,
            &markup,
            msg_label.clone(),
            None,
        );

        self.connect_right_click_menu(msg, Some(&msg_label));
        msg_label.set_markup(&format!("<b>{}</b> {}", sname, markup));
        self.set_label_styles(&msg_label);

        bx.add(&msg_label);
        bx
    }

    fn connect_right_click_menu(&self, msg: &Message, label: Option<&gtk::Label>) -> Option<()> {
        let id = msg.id.clone();
        let mtype = msg.mtype.clone();
        let redactable = msg.redactable.clone();
        let eventbox_weak = self.eventbox.downgrade();
        let widget = if let Some(l) = label {
            l.upcast_ref::<gtk::Widget>()
        } else {
            self.eventbox.upcast_ref::<gtk::Widget>()
        };

        let evbox = eventbox_weak.clone();
        let i = id.clone();
        widget.connect_button_press_event(move |w, e| {
            if e.get_button() == 3 {
                let eventbox = upgrade_weak!(evbox, gtk::Inhibit(false));
                MessageMenu::new(i.as_str(), &mtype, &redactable, &eventbox, w);
                Inhibit(true)
            } else {
                Inhibit(false)
            }
        });

        let widget_weak = widget.downgrade();
        self.gesture.connect_pressed(move |_, _, _| {
            let eventbox = upgrade_weak!(eventbox_weak);
            let widget = upgrade_weak!(widget_weak);

            MessageMenu::new(&id, &mtype, &redactable, &eventbox, &widget);
        });
        None
    }

    fn connect_image(&self, msg: &Message) -> Option<()> {
        let data = glib::Variant::from(msg.id.as_str());
        self.row.set_action_name("app.open-media-viewer");
        self.row.set_action_target_value(&data);
        None
    }
}

fn highlight_username(
    label: gtk::Label,
    attr: &pango::AttrList,
    alias: &String,
    input: String,
) -> Option<()> {
    fn contains((start, end): (i32, i32), item: i32) -> bool {
        if start <= end {
            start <= item && end > item
        } else {
            start <= item || end > item
        }
    }

    let input = input.to_lowercase();
    let bounds = label.get_selection_bounds();
    let context = gtk::Widget::get_style_context(&label.clone().upcast::<gtk::Widget>())?;
    let fg = gtk::StyleContext::lookup_color(&context, "theme_selected_bg_color")?;
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
            if contains((mark_start, mark_end), bounds_start)
                && contains((mark_start, mark_end), bounds_end)
            {
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

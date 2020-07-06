use crate::i18n::i18n;
use itertools::Itertools;

use crate::backend::ThreadPool;
use crate::cache::CacheMap;
use chrono::prelude::*;
use fractal_api::identifiers::UserId;
use fractal_api::r0::AccessToken;
use fractal_api::url::Url;
use glib::clone;
use gtk::{prelude::*, ButtonExt, ContainerExt, LabelExt, Overlay, WidgetExt};
use std::cmp::max;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::util::markup_text;

use crate::cache::download_to_cache;
use crate::cache::download_to_cache_username;
use crate::cache::download_to_cache_username_emote;

use crate::globals;
use crate::uitypes::MessageContent as Message;
use crate::uitypes::RowType;
use crate::widgets;
use crate::widgets::message_menu::MessageMenu;
use crate::widgets::AvatarExt;
use crate::widgets::{AudioPlayerWidget, PlayerExt, VideoPlayerWidget};

/* A message row in the room history */
#[derive(Clone, Debug)]
pub struct MessageBox {
    access_token: AccessToken,
    server_url: Url,
    username: gtk::Label,
    pub username_event_box: gtk::EventBox,
    eventbox: gtk::EventBox,
    gesture: gtk::GestureLongPress,
    row: gtk::ListBoxRow,
    image: Option<gtk::DrawingArea>,
    video_player: Option<Rc<VideoPlayerWidget>>,
    pub header: bool,
}

impl MessageBox {
    pub fn new(server_url: Url, access_token: AccessToken) -> MessageBox {
        let username = gtk::Label::new(None);
        let eb = gtk::EventBox::new();
        let eventbox = gtk::EventBox::new();
        let row = gtk::ListBoxRow::new();
        let gesture = gtk::GestureLongPress::new(&eventbox);

        username.set_ellipsize(pango::EllipsizeMode::End);
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        gesture.set_touch_only(true);

        MessageBox {
            access_token,
            server_url,
            username,
            username_event_box: eb,
            eventbox,
            gesture,
            row,
            image: None,
            video_player: None,
            header: true,
        }
    }

    /* create the message row with or without a header */
    pub fn create(
        &mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        msg: &Message,
        has_header: bool,
        is_temp: bool,
    ) {
        self.set_msg_styles(msg, &self.row);
        self.row.set_selectable(false);
        let upload_attachment_msg = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let w = match msg.mtype {
            RowType::Emote => {
                self.row.set_margin_top(12);
                self.header = false;
                self.small_widget(thread_pool, msg)
            }
            RowType::Video if is_temp => {
                upload_attachment_msg
                    .add(&gtk::Label::new(Some(i18n("Uploading video.").as_str())));
                upload_attachment_msg
            }
            RowType::Audio if is_temp => {
                upload_attachment_msg
                    .add(&gtk::Label::new(Some(i18n("Uploading audio.").as_str())));
                upload_attachment_msg
            }
            RowType::Image if is_temp => {
                upload_attachment_msg
                    .add(&gtk::Label::new(Some(i18n("Uploading image.").as_str())));
                upload_attachment_msg
            }
            RowType::File if is_temp => {
                upload_attachment_msg.add(&gtk::Label::new(Some(i18n("Uploading file.").as_str())));
                upload_attachment_msg
            }
            _ if has_header => {
                self.row.set_margin_top(12);
                self.header = true;
                self.widget(thread_pool, user_info_cache, msg)
            }
            _ => {
                self.header = false;
                self.small_widget(thread_pool, msg)
            }
        };

        self.eventbox.add(&w);
        self.row.add(&self.eventbox);
        self.row.show_all();
        self.connect_right_click_menu(msg, None);
    }

    pub fn get_listbox_row(&self) -> &gtk::ListBoxRow {
        &self.row
    }

    pub fn tmpwidget(
        mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        msg: &Message,
    ) -> MessageBox {
        self.create(thread_pool, user_info_cache, msg, true, true);
        {
            let w = self.get_listbox_row();
            w.get_style_context().add_class("msg-tmp");
        }
        self
    }

    pub fn update_header(
        &mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        msg: Message,
        has_header: bool,
    ) {
        let w = if has_header && msg.mtype != RowType::Emote {
            self.row.set_margin_top(12);
            self.header = true;
            self.widget(thread_pool, user_info_cache, &msg)
        } else {
            if let RowType::Emote = msg.mtype {
                self.row.set_margin_top(12);
            }
            self.header = false;
            self.small_widget(thread_pool, &msg)
        };
        if let Some(eb) = self.eventbox.get_child() {
            eb.destroy(); // clean the eventbox
        }
        self.eventbox.add(&w);
        self.row.show_all();
    }

    fn widget(
        &mut self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        msg: &Message,
    ) -> gtk::Box {
        // msg
        // +--------+---------+
        // | avatar | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let content = self.build_room_msg_content(thread_pool.clone(), msg, false);
        /* Todo: make build_room_msg_avatar() faster (currently ~1ms) */
        let avatar = self.build_room_msg_avatar(thread_pool, user_info_cache, msg);

        msg_widget.pack_start(&avatar, false, false, 0);
        msg_widget.pack_start(&content, true, true, 0);

        msg_widget
    }

    fn small_widget(&mut self, thread_pool: ThreadPool, msg: &Message) -> gtk::Box {
        // msg
        // +--------+---------+
        // |        | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        let content = self.build_room_msg_content(thread_pool, msg, true);
        content.set_margin_start(50);

        msg_widget.pack_start(&content, true, true, 0);

        msg_widget
    }

    fn build_room_msg_content(
        &mut self,
        thread_pool: ThreadPool,
        msg: &Message,
        small: bool,
    ) -> gtk::Box {
        // content
        // +---------+
        // | info    |
        // +---------+
        // | body_bx |
        // +---------+
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);

        if !small {
            let info = self.build_room_msg_info(msg);
            info.set_margin_top(2);
            info.set_margin_bottom(3);
            content.pack_start(&info, false, false, 0);
        }

        let body_bx = self.build_room_msg_body_bx(thread_pool, msg);
        content.pack_start(&body_bx, true, true, 0);

        content
    }

    fn build_room_msg_body_bx(&mut self, thread_pool: ThreadPool, msg: &Message) -> gtk::Box {
        // body_bx
        // +------+-----------+
        // | body | edit_mark |
        // +------+-----------+
        let body_bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let body = match msg.mtype {
            RowType::Sticker => self.build_room_msg_sticker(thread_pool, msg),
            RowType::Image => self.build_room_msg_image(thread_pool, msg),
            RowType::Emote => self.build_room_msg_emote(msg),
            RowType::Audio => self.build_room_audio_player(thread_pool, msg),
            RowType::Video => self.build_room_video_player(thread_pool, msg),
            RowType::File => self.build_room_msg_file(msg),
            _ => self.build_room_msg_body(msg),
        };

        body_bx.pack_start(&body, true, true, 0);

        if let Some(replace_date) = msg.replace_date {
            let edit_mark = gtk::Image::new_from_icon_name(
                Some("document-edit-symbolic"),
                gtk::IconSize::Button,
            );
            edit_mark.get_style_context().add_class("edit-mark");
            edit_mark.set_valign(gtk::Align::End);

            let edit_tooltip = replace_date.format(&i18n("Last edited %c")).to_string();
            edit_mark.set_tooltip_text(Some(&edit_tooltip));

            body_bx.pack_start(&edit_mark, false, false, 0);
        }
        body_bx
    }

    fn build_room_msg_avatar(
        &self,
        thread_pool: ThreadPool,
        user_info_cache: Arc<Mutex<CacheMap<UserId, (String, String)>>>,
        msg: &Message,
    ) -> widgets::Avatar {
        let uid = msg.sender.clone();
        let alias = msg.sender_name.clone();
        let avatar = widgets::Avatar::avatar_new(Some(globals::MSG_ICON_SIZE));

        let data = avatar.circle(
            uid.to_string(),
            alias.clone(),
            globals::MSG_ICON_SIZE,
            None,
            None,
        );
        if let Some(name) = alias {
            self.username.set_text(&name);
        } else {
            self.username.set_text(&uid.to_string());
        }

        download_to_cache(
            thread_pool,
            user_info_cache,
            self.server_url.clone(),
            uid.clone(),
            data.clone(),
        );
        download_to_cache_username(
            self.server_url.clone(),
            self.access_token.clone(),
            uid,
            self.username.clone(),
            Some(data),
        );

        avatar
    }

    fn build_room_msg_username(&self, uname: String) -> gtk::Label {
        self.username.set_text(&uname);
        self.username.set_justify(gtk::Justification::Left);
        self.username.set_halign(gtk::Align::Start);
        self.username.get_style_context().add_class("username");

        self.username.clone()
    }

    /* Add classes to the widget based on message type */
    fn set_msg_styles(&self, msg: &Message, w: &gtk::ListBoxRow) {
        let style = w.get_style_context();
        match msg.mtype {
            RowType::Mention => style.add_class("msg-mention"),
            RowType::Emote => style.add_class("msg-emote"),
            RowType::Emoji => style.add_class("msg-emoji"),
            _ => {}
        }
    }

    fn set_label_styles(&self, w: &gtk::Label) {
        w.set_line_wrap(true);
        w.set_line_wrap_mode(pango::WrapMode::WordChar);
        w.set_justify(gtk::Justification::Left);
        w.set_xalign(0.0);
        w.set_valign(gtk::Align::Start);
        w.set_halign(gtk::Align::Fill);
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
                            highlight_username(w.clone(), &attr, &light, text.to_string());
                        }
                        w.set_attributes(Some(&attr));
                    }
                });

                let highlights = msg.highlights.clone();
                part.connect_property_selection_bound_notify(move |w| {
                    if let Some(text) = w.get_text() {
                        let attr = pango::AttrList::new();
                        for light in highlights.clone() {
                            highlight_username(w.clone(), &attr, &light, text.to_string());
                        }
                        w.set_attributes(Some(&attr));
                    }
                });

                if let Some(text) = part.get_text() {
                    let attr = pango::AttrList::new();
                    for light in msg.highlights.clone() {
                        highlight_username(part.clone(), &attr, &light, text.to_string());
                    }
                    part.set_attributes(Some(&attr));
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
        let msg_part = gtk::Label::new(None);
        msg_part.set_markup(&markup_text(body));
        self.set_label_styles(&msg_part);

        if k == MsgPartType::Quote {
            msg_part.get_style_context().add_class("quote");
        }
        msg_part
    }

    fn build_room_msg_image(&mut self, thread_pool: ThreadPool, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let img_path = match msg.thumb {
            // If the thumbnail is not a valid URL we use the msg.url
            Some(ref m) if m.starts_with("mxc:") || m.starts_with("http") => m.clone(),
            _ => msg.url.clone().unwrap_or_default(),
        };
        let image = widgets::image::Image::new(self.server_url.clone(), &img_path)
            .size(Some(globals::MAX_IMAGE_SIZE))
            .build(thread_pool);

        image.widget.get_style_context().add_class("image-widget");

        bx.pack_start(&image.widget, true, true, 0);
        bx.show_all();
        self.image = Some(image.widget);
        self.connect_media_viewer(msg);

        bx
    }

    fn build_room_msg_sticker(&self, thread_pool: ThreadPool, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        if let Some(url) = msg.url.as_ref() {
            let image = widgets::image::Image::new(self.server_url.clone(), url)
                .size(Some(globals::MAX_STICKER_SIZE))
                .build(thread_pool);
            image.widget.set_tooltip_text(Some(&msg.body[..]));

            bx.add(&image.widget);
        }

        bx
    }

    fn build_room_audio_player(&self, thread_pool: ThreadPool, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let player = AudioPlayerWidget::new();
        let start_playing = false;
        PlayerExt::initialize_stream(
            &player,
            &msg.url.clone().unwrap_or_default(),
            &self.server_url,
            thread_pool,
            &bx,
            start_playing,
        );

        let download_btn =
            gtk::Button::new_from_icon_name(Some("document-save-symbolic"), gtk::IconSize::Button);
        download_btn.set_tooltip_text(Some(i18n("Save").as_str()));

        let evid = msg
            .id
            .as_ref()
            .map(|evid| evid.to_string())
            .unwrap_or_default();
        let data = glib::Variant::from(evid);
        download_btn.set_action_target_value(Some(&data));
        download_btn.set_action_name(Some("message.save_as"));

        let control_box = PlayerExt::get_controls_container(&player)
            .expect("Every AudioPlayer must have controls.");
        bx.pack_start(&control_box, false, true, 0);
        bx.pack_start(&download_btn, false, false, 3);

        let outer_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let file_name = gtk::Label::new(Some(&format!("<b>{}</b>", msg.body)));
        file_name.set_use_markup(true);
        file_name.set_xalign(0.0);
        file_name.set_line_wrap(true);
        file_name.set_line_wrap_mode(pango::WrapMode::WordChar);
        outer_box.pack_start(&file_name, false, false, 0);
        outer_box.pack_start(&bx, false, false, 0);
        outer_box.get_style_context().add_class("audio-box");
        outer_box
    }

    fn build_room_video_player(&mut self, thread_pool: ThreadPool, msg: &Message) -> gtk::Box {
        let with_controls = false;
        let player = VideoPlayerWidget::new(with_controls);
        let bx = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let start_playing = false;
        PlayerExt::initialize_stream(
            &player,
            &msg.url.clone().unwrap_or_default(),
            &self.server_url,
            thread_pool,
            &bx,
            start_playing,
        );

        let overlay = Overlay::new();
        let video_widget = player.get_video_widget();
        video_widget.set_size_request(-1, 390);
        VideoPlayerWidget::auto_adjust_video_dimensions(&player);
        overlay.add(&video_widget);

        let play_button = gtk::Button::new();
        let play_icon = gtk::Image::new_from_icon_name(
            Some("media-playback-start-symbolic"),
            gtk::IconSize::Dialog,
        );
        play_button.set_image(Some(&play_icon));
        play_button.set_halign(gtk::Align::Center);
        play_button.set_valign(gtk::Align::Center);
        play_button.get_style_context().add_class("osd");
        play_button.get_style_context().add_class("play-icon");
        play_button.get_style_context().add_class("flat");
        let evid = msg
            .id
            .as_ref()
            .map(|evid| evid.to_string())
            .unwrap_or_default();
        let data = glib::Variant::from(evid);
        play_button.set_action_name(Some("app.open-media-viewer"));
        play_button.set_action_target_value(Some(&data));
        overlay.add_overlay(&play_button);

        let menu_button = gtk::MenuButton::new();
        let three_dot_icon =
            gtk::Image::new_from_icon_name(Some("view-more-symbolic"), gtk::IconSize::Button);
        menu_button.set_image(Some(&three_dot_icon));
        menu_button.get_style_context().add_class("osd");
        menu_button.get_style_context().add_class("round-button");
        menu_button.get_style_context().add_class("flat");
        menu_button.set_margin_top(12);
        menu_button.set_margin_end(12);
        menu_button.set_opacity(0.8);
        menu_button.set_halign(gtk::Align::End);
        menu_button.set_valign(gtk::Align::Start);
        menu_button.connect_size_allocate(|button, allocation| {
            let diameter = max(allocation.width, allocation.height);
            button.set_size_request(diameter, diameter);
        });
        overlay.add_overlay(&menu_button);

        let evid = msg.id.as_ref();
        let redactable = msg.redactable;
        let menu = MessageMenu::new(evid, &RowType::Video, &redactable, None, None);
        menu_button.set_popover(Some(&menu.get_popover()));

        bx.pack_start(&overlay, true, true, 0);
        self.connect_media_viewer(msg);
        self.video_player = Some(player);
        bx
    }

    pub fn get_video_widget(&self) -> Option<Rc<VideoPlayerWidget>> {
        self.video_player.clone()
    }

    fn build_room_msg_file(&self, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let btn_bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let name = msg.body.as_str();
        let name_lbl = gtk::Label::new(Some(name));
        name_lbl.set_tooltip_text(Some(name));
        name_lbl.set_ellipsize(pango::EllipsizeMode::End);

        name_lbl.get_style_context().add_class("msg-highlighted");

        let download_btn =
            gtk::Button::new_from_icon_name(Some("document-save-symbolic"), gtk::IconSize::Button);
        download_btn.set_tooltip_text(Some(i18n("Save").as_str()));

        let evid = msg
            .id
            .as_ref()
            .map(|evid| evid.to_string())
            .unwrap_or_default();

        let data = glib::Variant::from(&evid);
        download_btn.set_action_target_value(Some(&data));
        download_btn.set_action_name(Some("message.save_as"));

        let open_btn =
            gtk::Button::new_from_icon_name(Some("document-open-symbolic"), gtk::IconSize::Button);
        open_btn.set_tooltip_text(Some(i18n("Open").as_str()));

        let data = glib::Variant::from(&evid);
        open_btn.set_action_target_value(Some(&data));
        open_btn.set_action_name(Some("message.open_with"));

        btn_bx.pack_start(&open_btn, false, false, 0);
        btn_bx.pack_start(&download_btn, false, false, 0);
        btn_bx.get_style_context().add_class("linked");

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

        let date = gtk::Label::new(None);
        date.set_markup(&format!("<span alpha=\"60%\">{}</span>", d.trim()));
        date.set_line_wrap(true);
        date.set_justify(gtk::Justification::Right);
        date.set_valign(gtk::Align::Start);
        date.set_halign(gtk::Align::End);
        date.get_style_context().add_class("timestamp");

        date
    }

    fn build_room_msg_info(&self, msg: &Message) -> gtk::Box {
        // info
        // +----------+------+
        // | username | date |
        // +----------+------+
        let info = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let username = self.build_room_msg_username(
            msg.sender_name
                .clone()
                .unwrap_or_else(|| msg.sender.to_string()),
        );
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
            .unwrap_or_else(|| msg.sender.to_string());
        let msg_label = gtk::Label::new(None);
        let body: &str = &msg.body;
        let markup = markup_text(body);

        download_to_cache_username_emote(
            self.server_url.clone(),
            self.access_token.clone(),
            msg.sender.clone(),
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
        let mtype = msg.mtype;
        let redactable = msg.redactable;
        let widget = if let Some(l) = label {
            l.upcast_ref::<gtk::Widget>()
        } else {
            self.eventbox.upcast_ref::<gtk::Widget>()
        };

        let eventbox = &self.eventbox;
        let id = msg.id.clone();
        widget.connect_button_press_event(
            clone!(@weak eventbox => @default-return Inhibit(false), move |w, e| {
                if e.get_button() == 3 {
                    MessageMenu::new(id.as_ref(), &mtype, &redactable, Some(&eventbox), Some(w));
                    Inhibit(true)
                } else {
                    Inhibit(false)
                }
            }),
        );

        let id = msg.id.clone();
        self.gesture
            .connect_pressed(clone!(@weak eventbox, @weak widget => move |_, _, _| {
                MessageMenu::new(
                    id.as_ref(),
                    &mtype,
                    &redactable,
                    Some(&eventbox),
                    Some(&widget),
                );
            }));
        None
    }

    fn connect_media_viewer(&self, msg: &Message) -> Option<()> {
        let evid = msg.id.as_ref()?.to_string();
        let data = glib::Variant::from(evid);
        self.row.set_action_name(Some("app.open-media-viewer"));
        self.row.set_action_target_value(Some(&data));
        None
    }
}

fn highlight_username(
    label: gtk::Label,
    attr: &pango::AttrList,
    alias: &str,
    input: String,
) -> Option<()> {
    fn contains((start, end): (i32, i32), item: i32) -> bool {
        if start <= end {
            start <= item && end > item
        } else {
            start <= item || end > item
        }
    }

    let mut input = input.to_lowercase();
    let bounds = label.get_selection_bounds();
    let context = label.get_style_context();
    let fg = context.lookup_color("theme_selected_bg_color")?;
    let red = fg.red * 65535. + 0.5;
    let green = fg.green * 65535. + 0.5;
    let blue = fg.blue * 65535. + 0.5;
    let color = pango::Attribute::new_foreground(red as u16, green as u16, blue as u16)?;

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
        removed_char += pos.1 as u32;
    }

    None
}

#[derive(PartialEq)]
enum MsgPartType {
    Normal,
    Quote,
}

fn kind_of_line(line: &&str) -> MsgPartType {
    if line.trim_start().starts_with('>') {
        MsgPartType::Quote
    } else {
        MsgPartType::Normal
    }
}

fn trim_start_quote(line: &str) -> &str {
    line.trim_start().get(1..).unwrap_or(line).trim_start()
}

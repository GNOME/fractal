use crate::backend::media;
use crate::backend::ThreadPool;
use fractal_api::r0::AccessToken;
use glib::clone;

use fragile::Fragile;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::i18n::i18n;
use fractal_api::identifiers::UserId;
use fractal_api::url::Url;
use gdk::*;
use glib::signal;
use glib::source::Continue;
use gtk::prelude::*;
use gtk::Overlay;

use crate::types::Message;
use crate::types::Room;

use crate::uitypes::RowType;
use crate::widgets::image;
use crate::widgets::message_menu::MessageMenu;
use crate::widgets::ErrorDialog;
use crate::widgets::PlayerExt;
use crate::widgets::{MediaPlayer, VideoPlayerWidget};
use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct MediaViewer {
    data: Rc<RefCell<Data>>,
    /* gtk widgets we need to have a reference to */
    pub builder: gtk::Builder,
}

#[derive(Debug)]
struct VideoWidget {
    player: Rc<VideoPlayerWidget>,
    inner_box: gtk::Overlay,
    outer_box: gtk::Box,
    auto_adjust_size_ids: Option<(glib::SignalHandlerId, glib::SignalHandlerId)>,
}

impl VideoWidget {
    fn set_fullscreen_mode(&mut self) {
        if let Some((dimension_id, size_id)) = self.auto_adjust_size_ids.take() {
            self.player.get_player().disconnect(dimension_id);
            self.outer_box.disconnect(size_id);
        }

        self.outer_box
            .set_child_packing(&self.inner_box, true, true, 0, gtk::PackType::Start);

        self.inner_box.set_valign(gtk::Align::Fill);
        self.inner_box.set_halign(gtk::Align::Fill);

        let bx = self.outer_box.clone();
        gtk::timeout_add(50, move || {
            bx.set_margin_top(0);
            bx.set_margin_bottom(0);
            Continue(false)
        });

        for widget in self.inner_box.get_children() {
            if widget.is::<gtk::Revealer>() {
                let control_box = widget
                    .downcast::<gtk::Revealer>()
                    .unwrap()
                    .get_child()
                    .expect("The control box has to be added to the control box reavealer.");
                control_box
                    .get_style_context()
                    .remove_class("window-control-box");
                control_box
                    .get_style_context()
                    .add_class("fullscreen-control-box");
            }
        }
    }

    fn set_window_mode(&mut self) {
        self.outer_box
            .set_child_packing(&self.inner_box, false, false, 0, gtk::PackType::Start);

        self.inner_box.set_valign(gtk::Align::Center);
        self.inner_box.set_halign(gtk::Align::Center);
        let ids = VideoPlayerWidget::auto_adjust_box_size_to_video_dimensions(
            &self.outer_box,
            &self.player,
        );
        self.auto_adjust_size_ids = Some(ids);

        for widget in self.inner_box.get_children() {
            if widget.is::<gtk::Revealer>() {
                let control_box = widget
                    .downcast::<gtk::Revealer>()
                    .unwrap()
                    .get_child()
                    .expect("The control box reavealer has to contain the control box.");
                control_box
                    .get_style_context()
                    .remove_class("fullscreen-control-box");
                control_box
                    .get_style_context()
                    .add_class("window-control-box");
            }
        }
    }
}

#[derive(Debug)]
enum Widget {
    Image(image::Image),
    Video(VideoWidget),
    None,
}

#[derive(Debug)]
struct Data {
    builder: gtk::Builder,
    main_window: gtk::Window,
    server_url: Url,
    access_token: AccessToken,
    uid: UserId,
    admins: HashMap<UserId, i32>,

    widget: Widget,
    media_list: Vec<Message>,
    current_media_index: usize,

    signal_id: Option<signal::SignalHandlerId>,
    prev_batch: Option<String>,
    loading_more_media: bool,
    loading_error: bool,
    no_more_media: bool,
    is_fullscreen: bool,
    double_click_handler_id: Option<glib::SignalHandlerId>,
}

impl Data {
    pub fn new(
        server_url: Url,
        access_token: AccessToken,
        media_list: Vec<Message>,
        current_media_index: usize,
        main_window: gtk::Window,
        builder: gtk::Builder,
        uid: UserId,
        admins: HashMap<UserId, i32>,
    ) -> Data {
        let is_fullscreen = main_window
            .clone()
            .get_window()
            .unwrap()
            .get_state()
            .contains(gdk::WindowState::FULLSCREEN);
        Data {
            media_list,
            current_media_index,
            prev_batch: None,
            loading_more_media: false,
            loading_error: false,
            no_more_media: false,
            widget: Widget::None,
            builder,
            server_url,
            access_token,
            uid,
            admins,
            main_window,
            signal_id: None,
            is_fullscreen,
            double_click_handler_id: None,
        }
    }

    pub fn enter_full_screen(&mut self, thread_pool: ThreadPool) {
        self.main_window.fullscreen();
        self.is_fullscreen = true;

        let media_viewer_headerbar_box = self
            .builder
            .get_object::<gtk::Box>("media_viewer_headerbar_box")
            .expect("Can't find media_viewer_headerbar_box in ui file.");
        let media_viewer_headerbar = self
            .builder
            .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
            .expect("Can't find media_viewer_headerbar in ui file.");
        let headerbar_revealer = self
            .builder
            .get_object::<gtk::Revealer>("headerbar_revealer")
            .expect("Can't find headerbar_revealer in ui file.");
        headerbar_revealer.add_events(gdk::EventMask::ENTER_NOTIFY_MASK);
        headerbar_revealer.add_events(gdk::EventMask::LEAVE_NOTIFY_MASK);

        media_viewer_headerbar_box.remove(&media_viewer_headerbar);

        let media_viewer_back_button = self
            .builder
            .get_object::<gtk::Button>("media_viewer_back_button")
            .expect("Can't find media_viewer_back_button in ui file.");

        media_viewer_headerbar.remove(&media_viewer_back_button);
        media_viewer_headerbar.set_show_close_button(false);

        let full_screen_button_icon = self
            .builder
            .get_object::<gtk::Image>("full_screen_button_icon")
            .expect("Can't find full_screen_button_icon in ui file.");
        full_screen_button_icon.set_property_icon_name(Some("view-restore-symbolic"));

        headerbar_revealer.add(&media_viewer_headerbar);

        match self.widget {
            Widget::Video(ref mut widget) => {
                widget.set_fullscreen_mode();
                let media_viewport = self
                    .builder
                    .get_object::<gtk::Viewport>("media_viewport")
                    .expect("Cant find media_viewport in ui file.");
                media_viewport.show();
            }
            _ => {
                self.redraw_media_in_viewport(thread_pool);
            }
        }
    }

    pub fn leave_full_screen(&mut self, thread_pool: ThreadPool) {
        self.main_window.unfullscreen();
        self.is_fullscreen = false;

        let media_viewer_headerbar_box = self
            .builder
            .get_object::<gtk::Box>("media_viewer_headerbar_box")
            .expect("Can't find media_viewer_headerbar_box in ui file.");
        let media_viewer_headerbar = self
            .builder
            .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
            .expect("Can't find media_viewer_headerbar in ui file.");
        let headerbar_revealer = self
            .builder
            .get_object::<gtk::Revealer>("headerbar_revealer")
            .expect("Can't find headerbar_revealer in ui file.");
        let media_viewer_back_button = self
            .builder
            .get_object::<gtk::Button>("media_viewer_back_button")
            .expect("Can't find media_viewer_back_button in ui file.");

        if let Some(ch) = headerbar_revealer.get_child() {
            headerbar_revealer.remove(&ch);
        }

        media_viewer_headerbar.pack_start(&media_viewer_back_button);
        media_viewer_headerbar.set_child_position(&media_viewer_back_button, 0);
        media_viewer_headerbar.set_show_close_button(true);

        let full_screen_button_icon = self
            .builder
            .get_object::<gtk::Image>("full_screen_button_icon")
            .expect("Can't find full_screen_button_icon in ui file.");
        full_screen_button_icon.set_property_icon_name(Some("view-fullscreen-symbolic"));

        media_viewer_headerbar.set_hexpand(true);
        media_viewer_headerbar_box.add(&media_viewer_headerbar);

        match self.widget {
            Widget::Video(ref mut widget) => {
                widget.set_window_mode();
                let media_viewport = self
                    .builder
                    .get_object::<gtk::Viewport>("media_viewport")
                    .expect("Cant find media_viewport in ui file.");
                media_viewport.show_all();
                if widget.player.is_playing() {
                    /* For some reason, if we don't replay the video, the play button,
                    which theoretically is hidden, appears next to the pause button. */
                    widget.player.pause();
                    widget.player.play();
                }
            }
            _ => {
                self.redraw_media_in_viewport(thread_pool);
            }
        }
    }

    pub fn set_nav_btn_visibility(&self) {
        let previous_media_button = self
            .builder
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");

        let next_media_button = self
            .builder
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");

        if self.current_media_index == 0 && self.no_more_media {
            previous_media_button.set_visible(false);
        } else {
            previous_media_button.set_visible(true);
        }

        if !self.media_list.is_empty() && self.current_media_index >= self.media_list.len() - 1 {
            next_media_button.set_visible(false);
        } else {
            next_media_button.set_visible(true);
        }
    }

    pub fn previous_media(&mut self, thread_pool: ThreadPool) -> bool {
        if self.no_more_media {
            return true;
        }

        if self.current_media_index == 0 {
            false
        } else {
            {
                self.current_media_index -= 1;
                let name = &self.media_list[self.current_media_index].body;
                set_header_title(&self.builder, name);
            }

            self.redraw_media_in_viewport(thread_pool);
            true
        }
    }

    pub fn next_media(&mut self, thread_pool: ThreadPool) {
        if self.current_media_index >= self.media_list.len() - 1 {
            return;
        }
        {
            self.current_media_index += 1;
            let name = &self.media_list[self.current_media_index].body;
            set_header_title(&self.builder, name);
        }

        self.redraw_media_in_viewport(thread_pool);
    }

    pub fn redraw_media_in_viewport(&mut self, thread_pool: ThreadPool) {
        let media_viewport = self
            .builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");

        if let Some(child) = media_viewport.get_child() {
            media_viewport.remove(&child);
        }

        let msg = &self.media_list[self.current_media_index];
        let url = msg.url.clone().unwrap_or_default();
        match msg.mtype.as_ref() {
            "m.image" => {
                let image = image::Image::new(self.server_url.clone(), &url)
                    .shrink_to_fit(true)
                    .center(true)
                    .build(thread_pool);
                media_viewport.add(&image.widget);
                image.widget.show();
                self.widget = Widget::Image(image);
            }
            "m.video" => {
                let widget = self.create_video_widget(thread_pool, &url);
                media_viewport.add(&widget.outer_box);
                self.widget = Widget::Video(widget);
                media_viewport.show_all();
            }
            _ => {}
        }

        self.set_context_menu_popover(&msg);
        self.set_nav_btn_visibility();
    }

    fn create_video_widget(&self, thread_pool: ThreadPool, url: &str) -> VideoWidget {
        let with_controls = true;
        let player = VideoPlayerWidget::new(with_controls);
        let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let start_playing = true;
        PlayerExt::initialize_stream(
            &player,
            url,
            &self.server_url.clone(),
            thread_pool,
            &bx,
            start_playing,
        );

        let overlay = Overlay::new();
        overlay.add(&player.get_video_widget());

        let full_control_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        if self.is_fullscreen {
            full_control_box
                .get_style_context()
                .add_class("fullscreen-control-box");
        } else {
            full_control_box
                .get_style_context()
                .add_class("window-control-box");
        }

        let control_box = PlayerExt::get_controls_container(&player).unwrap();
        full_control_box.pack_start(&control_box, false, true, 0);

        let mute_button = gtk::Button::new_from_icon_name(
            Some("audio-volume-high-symbolic"),
            gtk::IconSize::Button,
        );
        /* The followign callback requires `Send` but is handled by the gtk main loop */
        let button = Fragile::new(mute_button.clone());
        PlayerExt::get_player(&player).connect_state_changed(move |player, state| match state {
            gst_player::PlayerState::Playing if player.get_mute() => {
                let image = gtk::Image::new_from_icon_name(
                    Some("audio-volume-muted-symbolic"),
                    gtk::IconSize::Button,
                );
                button.get().set_image(Some(&image));
            }
            _ => {}
        });
        mute_button.connect_clicked(clone!(@weak player => move |button| {
            VideoPlayerWidget::switch_mute_state(&player, &button);
        }));
        full_control_box.pack_start(&mute_button, false, false, 0);

        let control_revealer = gtk::Revealer::new();
        control_revealer.add(&full_control_box);
        control_revealer.set_reveal_child(true);
        let source_id: Rc<RefCell<Option<glib::source::SourceId>>> = Rc::new(RefCell::new(None));
        let first_sid = gtk::timeout_add_seconds(
            1,
            clone!(@strong source_id, @strong control_revealer => move || {
                control_revealer.set_reveal_child(false);
                *source_id.borrow_mut() = None;
                Continue(false)
            }),
        );
        *source_id.borrow_mut() = Some(first_sid);
        let media_viewer_box = self
            .builder
            .clone()
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Cant find media_viewer_box in ui file.");
        media_viewer_box.connect_motion_notify_event(clone!(
        @strong control_revealer,
        @strong source_id,
        @weak player
        => @default-return Inhibit(false), move |_, _| {
            control_revealer.set_reveal_child(true);
            if let Some(sid) = source_id.borrow_mut().take() {
                glib::source::source_remove(sid);
            }
            let new_sid = gtk::timeout_add_seconds(
                1,
                clone!(
                    @strong source_id,
                    @strong control_revealer
                    => move || {
                        if player.is_playing() {
                            control_revealer.set_reveal_child(false);
                        }
                        *source_id.borrow_mut() = None;
                        Continue(false)
                    }),
            );
            *source_id.borrow_mut() = Some(new_sid);

            Inhibit(false)
        }));

        control_revealer.set_valign(gtk::Align::End);
        control_revealer.get_style_context().add_class("osd");
        overlay.add_overlay(&control_revealer);

        bx.pack_start(&overlay, false, false, 0);

        bx.connect_state_flags_changed(clone!(@weak player => move |_, flag| {
            let focussed = gtk::StateFlags::BACKDROP;
            if !flag.contains(focussed) {
                player.pause();
            }
        }));

        self.main_window.connect_key_press_event(clone!(
        @strong control_revealer,
        @strong source_id,
        @weak player
        => @default-return Inhibit(false), move |_, k| {
            if player.get_video_widget().get_mapped() {
                if let gdk::enums::key::space = k.get_keyval() {
                    if player.is_playing() {
                        control_revealer.set_reveal_child(true);
                    } else {
                        let new_sid = gtk::timeout_add_seconds(
                            1,
                            clone!(
                                @strong source_id,
                                @strong control_revealer,
                                @weak player
                                => @default-return Continue(false), move || {
                                    if player.is_playing() {
                                        control_revealer.set_reveal_child(false);
                                    }
                                    *source_id.borrow_mut() = None;
                                    Continue(false)
                                }),
                        );
                        *source_id.borrow_mut() = Some(new_sid);
                    }
                    VideoPlayerWidget::switch_play_pause_state(&player);
                }
            }
            Inhibit(false)
        }));
        let ui = self.builder.clone();
        let media_viewer_box = ui
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Cant find media_viewer_box in ui file.");
        let click_timeout_id = Rc::new(RefCell::new(None));
        media_viewer_box.connect_button_press_event(clone!(
        @strong control_revealer,
        @strong source_id,
        @weak player
        => @default-return Inhibit(false), move |_, e| {
            let source_id = source_id.clone();
            let revealer = control_revealer.clone();
            if let EventType::ButtonPress = e.get_event_type() {
                if click_timeout_id.borrow().is_some() {
                    let id = click_timeout_id.borrow_mut().take().unwrap();
                    glib::source::source_remove(id);
                } else {
                    let sid = gtk::timeout_add(
                        250,
                        clone!(@strong player, @strong click_timeout_id => move || {
                            if player.is_playing() {
                                revealer.set_reveal_child(true);
                            } else {
                                let new_sid = gtk::timeout_add_seconds(
                                    1,
                                    clone!(
                                        @strong source_id,
                                        @strong revealer,
                                        @weak player
                                        => @default-return Continue(false), move || {
                                            if player.is_playing() {
                                                revealer.set_reveal_child(false);
                                            }
                                            *source_id.borrow_mut() = None;
                                            Continue(false)
                                        }),
                                );
                                *source_id.borrow_mut() = Some(new_sid);
                            }
                            VideoPlayerWidget::switch_play_pause_state(&player);

                            *click_timeout_id.borrow_mut() = None;
                            Continue(false)
                        }),
                    );
                    *click_timeout_id.borrow_mut() = Some(sid);
                }
            }
            Inhibit(false)
        }));

        let mut widget = VideoWidget {
            player,
            inner_box: overlay,
            outer_box: bx,
            auto_adjust_size_ids: None,
        };

        if self.is_fullscreen {
            widget.set_fullscreen_mode();
        } else {
            widget.set_window_mode();
        }

        widget
    }

    fn set_context_menu_popover(&self, msg: &Message) {
        let mtype = match msg.mtype.as_ref() {
            "m.image" => RowType::Image,
            "m.video" => RowType::Video,
            _ => {
                panic!("Data in the media viewer has to be of type image or video.");
            }
        };
        let admin = self.admins.get(&self.uid).copied().unwrap_or_default();
        let redactable = admin != 0 || self.uid == msg.sender;
        let event_id = msg.id.as_ref();
        let menu = MessageMenu::new(event_id, &mtype, &redactable, None, None);
        let popover = &menu.get_popover();
        let menu_button = self
            .builder
            .get_object::<gtk::MenuButton>("media_viewer_menu_button")
            .expect("Can't find media_viewer_menu_button in ui file.");
        menu_button.set_popover(Some(popover));
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        if let Some(signal_handler_id) = self.double_click_handler_id.take() {
            self.main_window.disconnect(signal_handler_id);
        }
        if let Widget::Video(widget) = &self.widget {
            widget.player.stop();
        }
    }
}

impl MediaViewer {
    pub fn new(
        main_window: gtk::Window,
        room: &Room,
        current_media_msg: &Message,
        server_url: Url,
        access_token: AccessToken,
        uid: UserId,
    ) -> MediaViewer {
        let builder = gtk::Builder::new();
        builder
            .add_from_resource("/org/gnome/Fractal/ui/media_viewer.ui")
            .expect("Can't load ui file: media_viewer.ui");

        let media_list: Vec<Message> = room
            .messages
            .clone()
            .into_iter()
            .filter(|msg| msg.mtype == "m.image" || msg.mtype == "m.video")
            .collect();

        let current_media_index = media_list
            .iter()
            .position(|media| media.id == current_media_msg.id)
            .unwrap_or_default();

        MediaViewer {
            data: Rc::new(RefCell::new(Data::new(
                server_url,
                access_token,
                media_list,
                current_media_index,
                main_window,
                builder.clone(),
                uid,
                room.admins.clone(),
            ))),
            builder,
        }
    }

    pub fn create(&mut self, thread_pool: ThreadPool) -> Option<(gtk::Box, gtk::Box)> {
        let body = self
            .builder
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Can't find media_viewer_box in ui file.");
        let header = self
            .builder
            .get_object::<gtk::Box>("media_viewer_headerbar_box")
            .expect("Can't find media_viewer_headerbar in ui file.");
        self.connect_media_viewer_headerbar(thread_pool.clone());
        self.connect_media_viewer_box(thread_pool);
        self.connect_stop_video_when_leaving();

        Some((body, header))
    }

    pub fn display_media_viewer(&mut self, thread_pool: ThreadPool, media_msg: Message) {
        let previous_media_revealer = self
            .builder
            .get_object::<gtk::Revealer>("previous_media_revealer")
            .expect("Cant find previous_media_revealer in ui file.");
        previous_media_revealer.set_reveal_child(false);

        let next_media_revealer = self
            .builder
            .get_object::<gtk::Revealer>("next_media_revealer")
            .expect("Cant find next_media_revealer in ui file.");
        next_media_revealer.set_reveal_child(false);

        set_header_title(&self.builder, &media_msg.body);

        let media_viewport = self
            .builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");

        let url = media_msg.url.clone().unwrap_or_default();
        match media_msg.mtype.as_ref() {
            "m.image" => {
                let image = image::Image::new(self.data.borrow().server_url.clone(), &url)
                    .shrink_to_fit(true)
                    .center(true)
                    .build(thread_pool);

                media_viewport.add(&image.widget);
                media_viewport.show_all();

                self.data.borrow_mut().widget = Widget::Image(image);
            }
            "m.video" => {
                let video_widget = self.data.borrow().create_video_widget(thread_pool, &url);
                media_viewport.add(&video_widget.outer_box);
                media_viewport.show_all();

                self.data.borrow_mut().widget = Widget::Video(video_widget);
            }
            _ => {}
        }

        self.data.borrow().set_context_menu_popover(&media_msg);
        self.data.borrow_mut().set_nav_btn_visibility();
    }

    /* connect media viewer headerbar */
    pub fn connect_media_viewer_headerbar(&self, thread_pool: ThreadPool) {
        let own = &self.data;
        let full_screen_button = self
            .builder
            .get_object::<gtk::Button>("full_screen_button")
            .expect("Cant find full_screen_button in ui file.");
        full_screen_button.connect_clicked(clone!(@weak own => move |_| {
            let main_window = own.borrow().main_window.clone();
            if let Some(win) = main_window.get_window() {
                if !win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                    own.borrow_mut().enter_full_screen(thread_pool.clone());
                } else {
                    own.borrow_mut().leave_full_screen(thread_pool.clone())
                }
            }
        }));
    }

    pub fn connect_media_viewer_box(&self, thread_pool: ThreadPool) {
        let full_screen_button = self
            .builder
            .get_object::<gtk::Button>("full_screen_button")
            .expect("Cant find full_screen_button in ui file.");
        let id = self
            .data
            .borrow()
            .main_window
            .connect_button_press_event(move |_, e| {
                if let EventType::DoubleButtonPress = e.get_event_type() {
                    full_screen_button.clicked();
                }
                Inhibit(false)
            });
        self.data.borrow_mut().double_click_handler_id = Some(id);

        let header_hovered: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let nav_hovered: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let ui = self.builder.clone();
        let headerbar_revealer = ui
            .get_object::<gtk::Revealer>("headerbar_revealer")
            .expect("Can't find headerbar_revealer in ui file.");

        headerbar_revealer.connect_enter_notify_event(
            clone!(@strong header_hovered => move |_, _| {
                *(header_hovered.lock().unwrap()) = true;

                Inhibit(false)
            }),
        );

        headerbar_revealer.connect_leave_notify_event(
            clone!(@strong header_hovered => move |_, _| {
                *(header_hovered.lock().unwrap()) = false;

                Inhibit(false)
            }),
        );

        let previous_media_button = ui
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");

        previous_media_button.connect_enter_notify_event(
            clone!(@strong nav_hovered => move |_, _| {
                *(nav_hovered.lock().unwrap()) = true;

                Inhibit(false)
            }),
        );
        previous_media_button.connect_leave_notify_event(
            clone!(@strong nav_hovered => move |_, _| {
                *(nav_hovered.lock().unwrap()) = false;

                Inhibit(false)
            }),
        );

        let next_media_button = ui
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");

        next_media_button.connect_enter_notify_event(clone!(
        @strong nav_hovered
        => move |_, _| {
            *(nav_hovered.lock().unwrap()) = true;

            Inhibit(false)
        }));
        next_media_button.connect_leave_notify_event(clone!(
        @strong nav_hovered
        => move |_, _| {
            *(nav_hovered.lock().unwrap()) = false;

            Inhibit(false)
        }));

        let source_id: Arc<Mutex<Option<glib::source::SourceId>>> = Arc::new(Mutex::new(None));
        let win = self.data.borrow().main_window.clone();
        self.data
            .borrow()
            .main_window
            .connect_motion_notify_event(move |_, _| {
                {
                    let mut id = source_id.lock().unwrap();
                    if let Some(sid) = id.take() {
                        glib::source::source_remove(sid);
                    }
                }

                gdk::Display::get_default()
                    .and_then(|disp| disp.get_default_seat())
                    .and_then(|seat| seat.get_pointer())
                    .map(|ptr| {
                        let win = win.get_window()?;
                        let (_, _, y, _) = win.get_device_position(&ptr);
                        if y <= 6 && win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                            headerbar_revealer.set_reveal_child(true);
                        }
                        Some(true)
                    });

                let previous_media_revealer = ui
                    .get_object::<gtk::Revealer>("previous_media_revealer")
                    .expect("Cant find previous_media_revealer in ui file.");
                previous_media_revealer.set_reveal_child(true);

                let next_media_revealer = ui
                    .get_object::<gtk::Revealer>("next_media_revealer")
                    .expect("Cant find next_media_revealer in ui file.");
                next_media_revealer.set_reveal_child(true);

                let sid = gtk::timeout_add(
                    1000,
                    clone!(
                    @strong ui,
                    @strong header_hovered,
                    @strong nav_hovered,
                    @strong source_id
                    => move || {
                        let menu_popover_is_visible = ui
                        .get_object::<gtk::MenuButton>("media_viewer_menu_button")
                        .expect("Can't find headerbar_revealer in ui file.")
                        .get_popover()
                        .filter(|p| p.get_visible())
                        .is_some();
                            if !*header_hovered.lock().unwrap() && !menu_popover_is_visible {
                                let headerbar_revealer = ui
                                    .get_object::<gtk::Revealer>("headerbar_revealer")
                                    .expect("Can't find headerbar_revealer in ui file.");
                                headerbar_revealer.set_reveal_child(false);
                            }
                        if !*nav_hovered.lock().unwrap() {
                            let previous_media_revealer = ui
                                .get_object::<gtk::Revealer>("previous_media_revealer")
                                .expect("Cant find previous_media_revealer in ui file.");
                            previous_media_revealer.set_reveal_child(false);

                            let next_media_revealer = ui
                                .get_object::<gtk::Revealer>("next_media_revealer")
                                .expect("Cant find next_media_revealer in ui file.");
                            next_media_revealer.set_reveal_child(false);
                        }

                        *(source_id.lock().unwrap()) = None;
                        Continue(false)
                    }),
                );

                *(source_id.lock().unwrap()) = Some(sid);
                Inhibit(false)
            });

        let data = &self.data;

        let builder = self.builder.clone();
        let previous_media_button = self
            .builder
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");
        let t_pool = thread_pool.clone();
        previous_media_button.connect_clicked(clone!(@weak data => move |_| {
            if !data.borrow_mut().previous_media(t_pool.clone()) {
                load_more_media(t_pool.clone(), data, builder.clone());
            }
        }));

        let next_media_button = self
            .builder
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");
        next_media_button.connect_clicked(clone!(@weak data => move |_| {
            data.borrow_mut().next_media(thread_pool.clone());
        }));
        let full_screen_button = self
            .builder
            .get_object::<gtk::Button>("full_screen_button")
            .expect("Cant find full_screen_button in ui file.");

        let id = self
            .data
            .borrow()
            .main_window
            .connect_key_press_event(move |w, k| {
                match k.get_keyval() {
                    gdk::enums::key::Escape => {
                        // leave full screen only if we're currently in fullscreen
                        if let Some(win) = w.get_window() {
                            if win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                                full_screen_button.clicked();
                                return Inhibit(true);
                            }
                        }

                        Inhibit(false)
                    }
                    gdk::enums::key::Left => {
                        previous_media_button.clicked();
                        Inhibit(true)
                    }
                    gdk::enums::key::Right => {
                        next_media_button.clicked();
                        Inhibit(true)
                    }
                    _ => Inhibit(false),
                }
            });
        self.data.borrow_mut().signal_id = Some(id);

        // Remove the keyboard signal management on hide
        let ui = self.builder.clone();
        let media_viewer_box = ui
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Cant find media_viewer_box in ui file.");
        media_viewer_box.connect_unmap(clone!(@weak data => move |_| {
            let id = data.borrow_mut().signal_id.take();
            let main_window = &data.borrow().main_window;
            if let Some(id) = id {
                signal::signal_handler_disconnect(main_window, id);
            }
        }));
    }

    fn connect_stop_video_when_leaving(&self) {
        let media_viewer_box = self
            .builder
            .clone()
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Cant find media_viewer_box in ui file.");
        let data_weak = Rc::downgrade(&self.data);
        media_viewer_box.connect_unmap(move |_| {
            if let Some(data) = data_weak.upgrade() {
                if let Widget::Video(widget) = &data.borrow().widget {
                    PlayerExt::get_player(&widget.player).stop();
                }
            }
        });
    }

    pub fn disconnect_signal_id(&mut self) {
        let id = self.data.borrow_mut().signal_id.take();
        let main_window = &self.data.borrow().main_window;
        if let Some(id) = id {
            signal::signal_handler_disconnect(main_window, id);
        }
    }
}

fn set_header_title(ui: &gtk::Builder, title: &str) {
    let media_viewer_headerbar = ui
        .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
        .expect("Cant find media_viewer_headerbar in ui file.");
    media_viewer_headerbar.set_title(Some(title));
}

fn loading_state(ui: &gtk::Builder, val: bool) -> bool {
    let notification: gtk::Revealer = ui
        .get_object("media_viewer_notify_revealer")
        .expect("Can't find media_viewer_notify_revealer in ui file.");
    notification.set_reveal_child(val);

    let previous_media_button = ui
        .get_object::<gtk::Button>("previous_media_button")
        .expect("Cant find previous_media_button in ui file.");
    previous_media_button.set_sensitive(!val);

    let next_media_button = ui
        .get_object::<gtk::Button>("next_media_button")
        .expect("Cant find next_media_button in ui file.");
    next_media_button.set_sensitive(!val);

    val
}

fn load_more_media(thread_pool: ThreadPool, data: Rc<RefCell<Data>>, builder: gtk::Builder) {
    data.borrow_mut().loading_more_media = loading_state(&builder, true);

    let msg = data.borrow().media_list[data.borrow().current_media_index].clone();
    let roomid = msg.room.clone();
    let first_media_id = unwrap_or_unit_return!(msg.id);
    let prev_batch = data.borrow().prev_batch.clone();
    let server_url = data.borrow().server_url.clone();
    let access_token = data.borrow().access_token.clone();

    let (tx, rx): (
        Sender<(Vec<Message>, String)>,
        Receiver<(Vec<Message>, String)>,
    ) = channel();
    media::get_media_list_async(
        thread_pool.clone(),
        server_url,
        access_token,
        roomid,
        first_media_id,
        prev_batch,
        tx,
    );

    let ui = builder.clone();
    gtk::timeout_add(
        50,
        clone!(
        @weak data
        => @default-return Continue(false), move || match rx.try_recv() {
            Err(TryRecvError::Empty) => Continue(true),
            Err(TryRecvError::Disconnected) => {
                data.borrow_mut().loading_error = true;
                let err = i18n("Error while loading previous media");
                ErrorDialog::new(false, &err);

                Continue(false)
            }
            Ok((msgs, prev_batch)) => {
                if msgs.is_empty() {
                    data.borrow_mut().no_more_media = true;
                    return Continue(false);
                }

                let thread_pool = thread_pool.clone();
                let media_list = data.borrow().media_list.clone();
                let img_msgs: Vec<Message> = msgs
                    .into_iter()
                    .filter(|msg| msg.mtype == "m.image" || msg.mtype == "m.video")
                    .collect();
                let img_msgs_count = img_msgs.len();
                let new_media_list: Vec<Message> =
                    img_msgs.into_iter().chain(media_list.into_iter()).collect();
                data.borrow_mut().media_list = new_media_list;
                data.borrow_mut().prev_batch = Some(prev_batch);
                if img_msgs_count == 0 {
                    load_more_media(thread_pool, data, builder.clone());
                } else {
                    data.borrow_mut().current_media_index += img_msgs_count;
                    data.borrow_mut().previous_media(thread_pool);
                    data.borrow_mut().loading_more_media = loading_state(&ui, false);
                }

                Continue(false)
            }
        }),
    );
}

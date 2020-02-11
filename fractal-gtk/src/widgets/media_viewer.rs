use fractal_api::clone;
use fractal_api::r0::AccessToken;
use gdk;

use std::cell::RefCell;
use std::rc::Rc;

use crate::i18n::i18n;
use dirs;
use gdk::*;
use glib;
use glib::signal;
use gtk;
use gtk::prelude::*;
use gtk::Overlay;
use gtk::ResponseType;
use url::Url;

use crate::types::Message;
use crate::types::Room;

use std::fs;

use crate::backend::BKCommand;
use crate::widgets::image;
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
    backend: Sender<BKCommand>,
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
        self.inner_box.set_margin_top(0);
        self.inner_box.set_margin_bottom(0);
        self.outer_box.set_margin_top(0);
        self.outer_box.set_margin_bottom(0);

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
    backend: Sender<BKCommand>,
    server_url: Url,
    access_token: AccessToken,

    widget: Widget,
    media_list: Vec<Message>,
    current_media_index: usize,

    signal_id: Option<signal::SignalHandlerId>,
    prev_batch: Option<String>,
    loading_more_media: bool,
    loading_error: bool,
    no_more_media: bool,
    is_fullscreen: bool,
}

impl Data {
    pub fn new(
        backend: Sender<BKCommand>,
        server_url: Url,
        access_token: AccessToken,
        media_list: Vec<Message>,
        current_media_index: usize,
        main_window: gtk::Window,
        builder: gtk::Builder,
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
            backend,
            server_url,
            access_token,
            main_window,
            signal_id: None,
            is_fullscreen,
        }
    }

    pub fn save_media(&self) {
        let local_path = match &self.widget {
            Widget::Image(image) => image.local_path.lock().unwrap().clone(),
            Widget::Video(widget) => widget.player.get_local_path_access().borrow().clone(),
            Widget::None => None,
        };
        if let Some(local_path) = local_path {
            save_file_as(
                &self.main_window,
                local_path,
                self.media_list[self.current_media_index].body.clone(),
            );
        } else {
            ErrorDialog::new(false, &i18n("Media is not loaded, yet."));
        }
    }

    pub fn enter_full_screen(&mut self) {
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
                self.redraw_media_in_viewport();
            }
        }
    }

    pub fn leave_full_screen(&mut self) {
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
                self.redraw_media_in_viewport();
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

        if self.media_list.len() > 0 && self.current_media_index >= self.media_list.len() - 1 {
            next_media_button.set_visible(false);
        } else {
            next_media_button.set_visible(true);
        }
    }

    pub fn previous_media(&mut self) -> bool {
        if self.no_more_media {
            return true;
        }

        if self.current_media_index == 0 {
            return false;
        } else {
            {
                self.current_media_index -= 1;
                let name = &self.media_list[self.current_media_index].body;
                set_header_title(&self.builder, name);
            }

            self.redraw_media_in_viewport();
            return true;
        }
    }

    pub fn next_media(&mut self) {
        if self.current_media_index >= self.media_list.len() - 1 {
            return;
        }
        {
            self.current_media_index += 1;
            let name = &self.media_list[self.current_media_index].body;
            set_header_title(&self.builder, name);
        }

        self.redraw_media_in_viewport();
    }

    pub fn redraw_media_in_viewport(&mut self) {
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
                let image = image::Image::new(&self.backend, self.server_url.clone(), &url)
                    .fit_to_width(true)
                    .center(true)
                    .build();
                media_viewport.add(&image.widget);
                image.widget.show();
                self.widget = Widget::Image(image);
            }
            "m.video" => {
                let widget = self.create_video_widget(&url);
                media_viewport.add(&widget.outer_box);
                self.widget = Widget::Video(widget);
                media_viewport.show_all();
            }
            _ => {}
        }
        self.set_nav_btn_visibility();
    }

    fn create_video_widget(&self, url: &String) -> VideoWidget {
        let with_controls = true;
        let player = VideoPlayerWidget::new(with_controls);
        let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let start_playing = true;
        PlayerExt::initialize_stream(
            &player,
            &self.backend,
            url,
            &self.server_url.clone(),
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
            gtk::IconSize::Button.into(),
        );
        let player_weak = Rc::downgrade(&player);
        mute_button.connect_clicked(move |button| {
            player_weak.upgrade().map(|player| {
                VideoPlayerWidget::switch_mute_state(&player, &button);
            });
        });
        full_control_box.pack_start(&mute_button, false, false, 0);

        let control_revealer = gtk::Revealer::new();
        control_revealer.add(&full_control_box);
        control_revealer.set_reveal_child(true);
        let source_id: Rc<RefCell<Option<glib::source::SourceId>>> = Rc::new(RefCell::new(None));
        let first_sid = gtk::timeout_add_seconds(
            1,
            clone!(source_id, control_revealer => move || {
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
        let player_weak = Rc::downgrade(&player);
        media_viewer_box.connect_motion_notify_event(
            clone!( control_revealer, source_id => move |_, _| {
                player_weak.upgrade().map(|player| {
                control_revealer.set_reveal_child(true);
                if let Some(sid) = source_id.borrow_mut().take() {
                    glib::source::source_remove(sid);
                }
                let new_sid = gtk::timeout_add_seconds(
                    1,
                    clone!(source_id, control_revealer => move || {
                            if player.is_playing() {
                                control_revealer.set_reveal_child(false);
                            }
                            *source_id.borrow_mut() = None;
                            Continue(false)
                        }),
                    );
                    *source_id.borrow_mut() = Some(new_sid);
                });
                Inhibit(false)
            }),
        );

        control_revealer.set_valign(gtk::Align::End);
        control_revealer.get_style_context().add_class("osd");
        overlay.add_overlay(&control_revealer);

        bx.pack_start(&overlay, false, false, 0);

        let player_weak = Rc::downgrade(&player);
        bx.connect_state_flags_changed(move |_, flag| {
            let focussed = gtk::StateFlags::BACKDROP;
            player_weak.upgrade().map(|player| {
                if !flag.contains(focussed) {
                    player.pause();
                }
            });
        });

        let player_weak = Rc::downgrade(&player);
        self.main_window.connect_key_press_event(
            clone!(control_revealer, source_id => move |_, k| {
            player_weak.upgrade().map(|player| {
                if player.get_video_widget().get_mapped() {
                    match k.get_keyval() {
                        gdk::enums::key::space => {
                            if player.is_playing() {
                                control_revealer.set_reveal_child(true);
                            } else {
                                let new_sid = gtk::timeout_add_seconds(
                                    1,
                                    clone!(source_id, control_revealer, player_weak => move || {
                                        player_weak.upgrade().map(|player| {
                                            if player.is_playing() {
                                                control_revealer.set_reveal_child(false);
                                            }
                                            *source_id.borrow_mut() = None;
                                        });
                                        Continue(false)
                                    }),
                                );
                                *source_id.borrow_mut() = Some(new_sid);
                            }
                            VideoPlayerWidget::switch_play_pause_state(&player);
                        }
                        _ => {}
                    }
                }
            });
            Inhibit(false)
            }),
        );
        let ui = self.builder.clone();
        let media_viewer_box = ui
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Cant find media_viewer_box in ui file.");
        let player_weak = Rc::downgrade(&player);
        let click_timeout_id = Rc::new(RefCell::new(None));
        media_viewer_box.connect_button_press_event(
            clone!(control_revealer, source_id => move |_, e| {
                let source_id = source_id.clone();
                let revealer = control_revealer.clone();
                let pw = player_weak.clone();
                player_weak
                    .upgrade()
                    .map(|player| match e.get_event_type() {
                        EventType::ButtonPress => {
                            if click_timeout_id.borrow().is_some() {
                                let id = click_timeout_id.borrow_mut().take().unwrap();
                                glib::source::source_remove(id);
                            } else {
                                let sid = gtk::timeout_add(
                                    250,
                                    clone!(player, click_timeout_id => move || {
                                        if player.is_playing() {
                                            revealer.set_reveal_child(true);
                                        } else {
                                            let new_sid = gtk::timeout_add_seconds(
                                                1,
                                                clone!(source_id, revealer, pw => move || {
                                                    pw.upgrade().map(|player| {
                                                        if player.is_playing() {
                                                            revealer.set_reveal_child(false);
                                                        }
                                                        *source_id.borrow_mut() = None;
                                                    });
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
                        _ => {}
                    });
                Inhibit(false)
            }),
        );

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
}

impl Drop for Data {
    fn drop(&mut self) {
        match &self.widget {
            Widget::Video(widget) => {
                widget.player.stop();
            }
            _ => {}
        }
    }
}

impl MediaViewer {
    pub fn new(
        backend: Sender<BKCommand>,
        main_window: gtk::Window,
        room: &Room,
        current_media_msg: &Message,
        server_url: Url,
        access_token: AccessToken,
    ) -> MediaViewer {
        let builder = gtk::Builder::new();
        builder
            .add_from_resource("/org/gnome/Fractal/ui/media_viewer_menu.ui")
            .expect("Can't load ui file: media_viewer_menu.ui");
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
                backend.clone(),
                server_url,
                access_token,
                media_list,
                current_media_index,
                main_window,
                builder.clone(),
            ))),
            builder,
            backend,
        }
    }

    pub fn create(&mut self) -> Option<(gtk::Box, gtk::Box)> {
        let body = self
            .builder
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Can't find media_viewer_box in ui file.");
        let header = self
            .builder
            .get_object::<gtk::Box>("media_viewer_headerbar_box")
            .expect("Can't find media_viewer_headerbar in ui file.");
        self.connect_media_viewer_headerbar();
        self.connect_media_viewer_box();
        self.connect_stop_video_when_leaving();

        Some((body, header))
    }

    pub fn display_media_viewer(&mut self, media_msg: Message) {
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
                let image =
                    image::Image::new(&self.backend, self.data.borrow().server_url.clone(), &url)
                        .fit_to_width(true)
                        .center(true)
                        .build();

                media_viewport.add(&image.widget);
                media_viewport.show_all();

                self.data.borrow_mut().widget = Widget::Image(image);
            }
            "m.video" => {
                let video_widget = self.data.borrow().create_video_widget(&url);
                media_viewport.add(&video_widget.outer_box);
                media_viewport.show_all();

                self.data.borrow_mut().widget = Widget::Video(video_widget);
            }
            _ => {}
        }
        self.data.borrow_mut().set_nav_btn_visibility();
    }

    /* connect media viewer headerbar */
    pub fn connect_media_viewer_headerbar(&self) {
        let own_weak = Rc::downgrade(&self.data);
        let full_screen_button = self
            .builder
            .get_object::<gtk::Button>("full_screen_button")
            .expect("Cant find full_screen_button in ui file.");
        full_screen_button.connect_clicked(move |_| {
            own_weak.upgrade().map(|own| {
                let main_window = own.borrow().main_window.clone();
                if let Some(win) = main_window.get_window() {
                    if !win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                        own.borrow_mut().enter_full_screen();
                    } else {
                        own.borrow_mut().leave_full_screen()
                    }
                }
            });
        });

        let save_as_button = self
            .builder
            .get_object::<gtk::ModelButton>("save_as_button")
            .expect("Cant find save_as_button in ui file.");
        let data_weak = Rc::downgrade(&self.data);
        save_as_button.connect_clicked(move |_| {
            data_weak.upgrade().map(|data| {
                data.borrow().save_media();
            });
        });
    }

    pub fn connect_media_viewer_box(&self) {
        let full_screen_button = self
            .builder
            .get_object::<gtk::Button>("full_screen_button")
            .expect("Cant find full_screen_button in ui file.");
        self.data
            .borrow()
            .main_window
            .connect_button_press_event(move |_, e| {
                match e.get_event_type() {
                    EventType::DoubleButtonPress => {
                        full_screen_button.clicked();
                    }
                    _ => {}
                }
                Inhibit(false)
            });

        let header_hovered: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let nav_hovered: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let ui = self.builder.clone();
        let headerbar_revealer = ui
            .get_object::<gtk::Revealer>("headerbar_revealer")
            .expect("Can't find headerbar_revealer in ui file.");

        headerbar_revealer.connect_enter_notify_event(clone!(header_hovered => move |_, _| {
            *(header_hovered.lock().unwrap()) = true;

            Inhibit(false)
        }));

        headerbar_revealer.connect_leave_notify_event(clone!(header_hovered => move |_, _| {
            *(header_hovered.lock().unwrap()) = false;

            Inhibit(false)
        }));

        let previous_media_button = ui
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");

        previous_media_button.connect_enter_notify_event(clone!(nav_hovered => move |_, _| {
            *(nav_hovered.lock().unwrap()) = true;

            Inhibit(false)
        }));
        previous_media_button.connect_leave_notify_event(clone!(nav_hovered => move |_, _| {
            *(nav_hovered.lock().unwrap()) = false;

            Inhibit(false)
        }));

        let next_media_button = ui
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");

        next_media_button.connect_enter_notify_event(clone!(nav_hovered => move |_, _| {
            *(nav_hovered.lock().unwrap()) = true;

            Inhibit(false)
        }));
        next_media_button.connect_leave_notify_event(clone!(nav_hovered => move |_, _| {
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
                    clone!(ui, header_hovered, nav_hovered, source_id => move || {
                        if !*header_hovered.lock().unwrap() {
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
                        gtk::Continue(false)
                    }),
                );

                *(source_id.lock().unwrap()) = Some(sid);
                Inhibit(false)
            });

        let own_weak = Rc::downgrade(&self.data);
        let builder = self.builder.clone();
        let backend = self.backend.clone();
        let previous_media_button = self
            .builder
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");
        previous_media_button.connect_clicked(move |_| {
            own_weak.upgrade().map(|own| {
                if !own.borrow_mut().previous_media() {
                    load_more_media(own.clone(), builder.clone(), backend.clone());
                }
            });
        });

        let own_weak = Rc::downgrade(&self.data);
        let next_media_button = self
            .builder
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");
        next_media_button.connect_clicked(move |_| {
            own_weak.upgrade().map(|own| {
                own.borrow_mut().next_media();
            });
        });
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
        let data_weak = Rc::downgrade(&self.data);
        let ui = self.builder.clone();
        let media_viewer_box = ui
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Cant find media_viewer_box in ui file.");
        media_viewer_box.connect_unmap(move |_| {
            data_weak.upgrade().map(|data| {
                let id = data.borrow_mut().signal_id.take();
                let main_window = &data.borrow().main_window;
                if let Some(id) = id {
                    signal::signal_handler_disconnect(main_window, id);
                }
            });
        });
    }

    fn connect_stop_video_when_leaving(&self) {
        let media_viewer_box = self
            .builder
            .clone()
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Cant find media_viewer_box in ui file.");
        let data_weak = Rc::downgrade(&self.data);
        media_viewer_box.connect_unmap(move |_| {
            data_weak.upgrade().map(|data| match &data.borrow().widget {
                Widget::Video(widget) => {
                    PlayerExt::get_player(&widget.player).stop();
                }
                _ => {}
            });
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

fn load_more_media(data: Rc<RefCell<Data>>, builder: gtk::Builder, backend: Sender<BKCommand>) {
    data.borrow_mut().loading_more_media = loading_state(&builder, true);

    let msg = data.borrow().media_list[data.borrow().current_media_index].clone();
    let roomid = msg.room.clone();
    let first_media_id = Some(msg.id.clone());
    let prev_batch = data.borrow().prev_batch.clone();
    let server_url = data.borrow().server_url.clone();
    let access_token = data.borrow().access_token.clone();

    let (tx, rx): (
        Sender<(Vec<Message>, String)>,
        Receiver<(Vec<Message>, String)>,
    ) = channel();
    backend
        .send(BKCommand::GetMediaListAsync(
            server_url,
            access_token,
            roomid,
            first_media_id,
            prev_batch,
            tx,
        ))
        .unwrap();

    let ui = builder.clone();
    let data_weak = Rc::downgrade(&data);
    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => {
            data_weak.clone().upgrade().map(|data| {
                data.borrow_mut().loading_error = true;
                let err = i18n("Error while loading previous media");
                ErrorDialog::new(false, &err);
            });

            gtk::Continue(false)
        }
        Ok((msgs, prev_batch)) => {
            if msgs.len() == 0 {
                data_weak.clone().upgrade().map(|data| {
                    data.borrow_mut().no_more_media = true;
                });
                return gtk::Continue(false);
            }
            data_weak.upgrade().map(|data| {
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
                    load_more_media(data.clone(), builder.clone(), backend.clone());
                } else {
                    data.borrow_mut().current_media_index += img_msgs_count;
                    data.borrow_mut().previous_media();
                    data.borrow_mut().loading_more_media = loading_state(&ui, false);
                }
            });

            gtk::Continue(false)
        }
    });
}

/* FIXME: The following two functions should be moved to a different file,
 * so that they can be used again in different locations */
fn save_file_as(main_window: &gtk::Window, src: String, name: String) {
    let file_chooser = gtk::FileChooserNative::new(
        Some(i18n("Save media as").as_str()),
        Some(main_window),
        gtk::FileChooserAction::Save,
        Some(i18n("_Save").as_str()),
        Some(i18n("_Cancel").as_str()),
    );

    file_chooser.set_current_folder(dirs::download_dir().unwrap_or_default());
    file_chooser.set_current_name(&name);

    file_chooser.connect_response(move |fcd, res| {
        if ResponseType::from(res) == ResponseType::Accept {
            if let Err(_) = fs::copy(src.clone(), fcd.get_filename().unwrap_or_default()) {
                let err = i18n("Could not save the file");
                ErrorDialog::new(false, &err);
            }
        }
    });

    file_chooser.run();
}

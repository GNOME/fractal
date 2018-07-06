extern crate glib;
extern crate gtk;

use self::gtk::prelude::*;

use i18n::i18n;
use uibuilder;
use app::App;
use appop::AppOp;
use appop::AppState;

use widgets::image;

use types::Message;
use types::Room;

use backend::BKCommand;

use std::sync::{Arc, RwLock};
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::TryRecvError;

const FLOATING_POINT_ERROR: f64 = 0.01;

#[derive(Clone)]
pub struct MediaViewer {
    media_list: Arc<RwLock<Vec<Message>>>,
    current_media_index: Arc<RwLock<usize>>,

    prev_batch: Arc<RwLock<Option<String>>>,
    pub loading_more_media: Arc<RwLock<bool>>,
    loading_error: Arc<RwLock<bool>>,
    pub no_more_media: Arc<RwLock<bool>>,

    image: image::Image,
    zoom_levels: Vec<f64>,
}

impl MediaViewer {
    pub fn new(room: &Room, current_media_msg: &Message, image: image::Image) -> MediaViewer {
        let media_list: Vec<Message> = room.messages.clone()
                                                    .into_iter()
                                                    .filter(|msg| msg.mtype == "m.image")
                                                    .collect();

        let current_media_index = media_list.iter().position(|media| {
            media.id.clone().map_or(false, |media_id| {
                current_media_msg.id.clone().map_or(false, |current_media_id| {
                    media_id == current_media_id
                })
            })
        }).unwrap_or_default();

        MediaViewer {
            media_list: Arc::new(RwLock::new(media_list)),
            current_media_index: Arc::new(RwLock::new(current_media_index)),
            prev_batch: Arc::new(RwLock::new(None)),
            loading_more_media: Arc::new(RwLock::new(false)),
            loading_error: Arc::new(RwLock::new(false)),
            no_more_media: Arc::new(RwLock::new(false)),
            image,
            zoom_levels: vec![0.025, 0.05, 0.1, 0.2, 0.33, 0.5, 0.67, 1.0,
                              1.33, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0,
                              10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0,
                              18.0, 19.0, 20.0],
        }
    }

    pub fn set_zoom_level(&self, zlvl: f64) {
        *self.image.zoom_level.lock().unwrap() = Some(zlvl);
        self.image.widget.queue_draw();
    }
}

impl AppOp {
    pub fn display_media_viewer(&mut self, media_msg: Message, room_id: String) {
        let rooms = self.rooms.clone();
        let r = rooms.get(&room_id).unwrap();

        let previous_media_revealer = self.ui.builder
            .get_object::<gtk::Revealer>("previous_media_revealer")
            .expect("Cant find previous_media_revealer in ui file.");
        previous_media_revealer.set_reveal_child(false);

        let next_media_revealer = self.ui.builder
            .get_object::<gtk::Revealer>("next_media_revealer")
            .expect("Cant find next_media_revealer in ui file.");
        next_media_revealer.set_reveal_child(false);

        self.set_state(AppState::MediaViewer);

        set_header_title(&self.ui, &media_msg.body);

        let media_viewport = self.ui.builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");

        let image = image::Image::new(&self.backend, &media_msg.url.clone().unwrap_or_default())
                        .fit_to_width(true)
                        .fixed(true).center(true).build();

        media_viewport.add(&image.widget);
        media_viewport.show_all();

        self.media_viewer = Some(MediaViewer::new(r, &media_msg, image.clone()));

        let ui = self.ui.clone();
        let zoom_level = image.zoom_level.clone();
        image.widget.connect_draw(move |_, _| {
            if let Some(zlvl) = *zoom_level.lock().unwrap() {
                update_zoom_entry(&ui, zlvl);
            }

            Inhibit(false)
        });

        self.set_nav_btn_visibility();
        self.set_zoom_btn_sensitivity();
    }

    pub fn hide_media_viewer(&mut self) {
        let media_viewport = self.ui.builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");
        if let Some(child) = media_viewport.get_child() {
            media_viewport.remove(&child);
        }

        self.set_state(AppState::Chat);

        self.media_viewer = None;
    }

    pub fn previous_media(&mut self) {
        if self.media_viewer.is_none() {
            return;
        }

        let mv = self.media_viewer.clone().unwrap();

        if *mv.no_more_media.read().unwrap() {
            return;
        }

        if *mv.current_media_index.read().unwrap() == 0 {
            self.load_more_media();

            if *mv.loading_error.read().unwrap() {
                let err = i18n("Error while loading previous media");
                APPOP!(show_error, (err));
            }
        } else {
            *mv.current_media_index.write().unwrap() -= 1;
            let name = &(*mv.media_list.read().unwrap())[*mv.current_media_index.read().unwrap()].body;
            set_header_title(&self.ui, name);

            self.update_media_viewport();
        }
    }

    pub fn next_media(&mut self) {
        if let Some(ref mut mv) = self.media_viewer {
            if *mv.current_media_index.read().unwrap() >= mv.media_list.read().unwrap().len() - 1 {
                return;
            }

            *mv.current_media_index.write().unwrap() += 1;
            let name = &(*mv.media_list.read().unwrap())[*mv.current_media_index.read().unwrap()].body;
            set_header_title(&self.ui, name);
        }

        self.update_media_viewport();
    }

    pub fn load_more_media(&mut self) {
        if let Some(ref mut mv) = self.media_viewer {
            loading_state(&self.ui, mv.loading_more_media.clone(), true);

            let msg = (*mv.media_list.read().unwrap())[*mv.current_media_index.read().unwrap()].clone();
            let roomid = msg.room.clone();
            let first_media_id = msg.id.clone();
            let prev_batch = mv.prev_batch.clone();

            let (tx, rx): (Sender<(Vec<Message>, String)>, Receiver<(Vec<Message>, String)>) = channel();
            self.backend.send(BKCommand::GetMediaListAsync(roomid, first_media_id,
                                                      prev_batch.read().unwrap().clone(), tx)).unwrap();

            let ui = self.ui.clone();
            let media_list_clone = mv.media_list.clone();
            let current_media_index_clone = mv.current_media_index.clone();
            let prev_batch_clone = mv.prev_batch.clone();
            let loading_more_media_clone = mv.loading_more_media.clone();
            let loading_error_clone = mv.loading_error.clone();
            let no_more_media_clone = mv.no_more_media.clone();
            gtk::timeout_add(50, move || match rx.try_recv() {
                Err(TryRecvError::Empty) => gtk::Continue(true),
                Err(TryRecvError::Disconnected) => {
                    *loading_error_clone.write().unwrap() = true;

                    gtk::Continue(false)
                },
                Ok((msgs, prev_batch)) => {
                    if msgs.len() == 0 {
                        *no_more_media_clone.write().unwrap() = true;

                        return gtk::Continue(false);
                    }

                    let img_msgs: Vec<Message> = msgs.into_iter().filter(|msg| msg.mtype == "m.image").collect();
                    let img_msgs_count = img_msgs.len();
                    let new_media_list: Vec<Message> = img_msgs.into_iter()
                                                               .chain(media_list_clone.read().unwrap().clone().into_iter())
                                                               .collect();

                    *media_list_clone.write().unwrap() = new_media_list;
                    *prev_batch_clone.write().unwrap() = Some(prev_batch);
                    *current_media_index_clone.write().unwrap() += img_msgs_count;

                    APPOP!(previous_media);

                    loading_state(&ui, loading_more_media_clone.clone(), false);

                    gtk::Continue(false)
                }
            });
        }
    }

    pub fn set_nav_btn_visibility(&self) {
        if let Some(ref mv) = self.media_viewer {
            let previous_media_button = self.ui.builder
                .get_object::<gtk::Button>("previous_media_button")
                .expect("Cant find previous_media_button in ui file.");

            let next_media_button = self.ui.builder
                .get_object::<gtk::Button>("next_media_button")
                .expect("Cant find next_media_button in ui file.");

            if *mv.current_media_index.read().unwrap() == 0 && *mv.no_more_media.read().unwrap() {
                previous_media_button.set_visible(false);
            } else {
                previous_media_button.set_visible(true);
            }

            if *mv.current_media_index.read().unwrap() >= mv.media_list.read().unwrap().len() - 1 {
                next_media_button.set_visible(false);
            } else {
                next_media_button.set_visible(true);
            }
        }
    }

    pub fn zoom_out(&self) {
        if let Some(ref mv) = self.media_viewer {
            let zoom_level = *mv.image.zoom_level.lock().unwrap();
            if zoom_level.is_none() ||
               zoom_level.unwrap() <= mv.zoom_levels[0] {
                return;
            }

            if let Some(new_zlvl) = mv.zoom_levels.iter()
                .filter(|zlvl| **zlvl < zoom_level.unwrap()).last() {
                    mv.set_zoom_level(*new_zlvl);
            }
        }

        self.set_zoom_btn_sensitivity();
    }

    pub fn zoom_in(&self) {
        if let Some(ref mv) = self.media_viewer {
            let zoom_level = *mv.image.zoom_level.lock().unwrap();
            if zoom_level.is_none() ||
               zoom_level.unwrap() >= mv.zoom_levels[mv.zoom_levels.len() - 1] {
                return;
            }

            if let Some(new_zlvl) = mv.zoom_levels.iter()
                .filter(|zlvl| **zlvl > zoom_level.unwrap()).nth(0) {
                    mv.set_zoom_level(*new_zlvl);
            }
        }

        self.set_zoom_btn_sensitivity();
    }

    pub fn change_zoom_level(&self) {
        if let Some(ref mv) = self.media_viewer {
            let zoom_entry = self.ui.builder
                .get_object::<gtk::EntryBuffer>("zoom_level")
                .expect("Cant find zoom_level in ui file.");

            match zoom_entry.get_text().trim().trim_right_matches('%').parse::<f64>() {
                Ok(zlvl) => mv.set_zoom_level(zlvl / 100.0),
                Err(_) => if let Some(zlvl) = *mv.image.zoom_level.lock().unwrap() {
                    update_zoom_entry(&self.ui, zlvl)
                },
            }
        }

        self.set_zoom_btn_sensitivity();
    }

    pub fn enter_full_screen(&mut self) {
        let main_window = self.ui.builder
            .get_object::<gtk::ApplicationWindow>("main_window")
            .expect("Cant find main_window in ui file.");
        main_window.fullscreen();

        let stack_header = self.ui.builder
            .get_object::<gtk::Stack>("headerbar_stack")
            .expect("Can't find headerbar_stack in ui file.");
        let media_viewer_headerbar_box = self.ui.builder
            .get_object::<gtk::Box>("media_viewer_headerbar_box")
            .expect("Can't find media_viewer_headerbar_box in ui file.");
        let headerbar_revealer = self.ui.builder
            .get_object::<gtk::Revealer>("headerbar_revealer")
            .expect("Can't find headerbar_revealer in ui file.");
        // gdk::EventMask::ENTER_NOTIFY_MASK = 4096
        headerbar_revealer.add_events(4096);
        // gdk::EventMask::LEAVE_NOTIFY_MASK = 8192
        headerbar_revealer.add_events(8192);

        stack_header.remove(&media_viewer_headerbar_box);

        let media_viewer_headerbar = self.ui.builder
            .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
            .expect("Can't find media_viewer_headerbar in ui file.");
        let media_viewer_back_button = self.ui.builder
            .get_object::<gtk::Button>("media_viewer_back_button")
            .expect("Can't find media_viewer_back_button in ui file.");

        media_viewer_headerbar.remove(&media_viewer_back_button);
        media_viewer_headerbar.set_show_close_button(false);

        let full_screen_button_icon = self.ui.builder
            .get_object::<gtk::Image>("full_screen_button_icon")
            .expect("Can't find full_screen_button_icon in ui file.");
        full_screen_button_icon.set_property_icon_name(Some("view-restore-symbolic"));

        headerbar_revealer.add(&media_viewer_headerbar_box);

        self.update_media_viewport();
    }

    pub fn leave_full_screen(&mut self) {
        let main_window = self.ui.builder
            .get_object::<gtk::ApplicationWindow>("main_window")
            .expect("Cant find main_window in ui file.");
        main_window.unfullscreen();

        let stack_header = self.ui.builder
            .get_object::<gtk::Stack>("headerbar_stack")
            .expect("Can't find headerbar_stack in ui file.");
        let media_viewer_headerbar_box = self.ui.builder
            .get_object::<gtk::Box>("media_viewer_headerbar_box")
            .expect("Can't find media_viewer_headerbar_box in ui file.");
        let headerbar_revealer = self.ui.builder
            .get_object::<gtk::Revealer>("headerbar_revealer")
            .expect("Can't find headerbar_revealer in ui file.");

        if let Some(ch) = headerbar_revealer.get_child() {
            headerbar_revealer.remove(&ch);
        }

        let media_viewer_headerbar = self.ui.builder
            .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
            .expect("Can't find media_viewer_headerbar in ui file.");
        let media_viewer_back_button = self.ui.builder
            .get_object::<gtk::Button>("media_viewer_back_button")
            .expect("Can't find media_viewer_back_button in ui file.");

        media_viewer_headerbar.pack_start(&media_viewer_back_button);
        media_viewer_headerbar.set_child_position(&media_viewer_back_button, 0);
        media_viewer_headerbar.set_show_close_button(true);

        let full_screen_button_icon = self.ui.builder
            .get_object::<gtk::Image>("full_screen_button_icon")
            .expect("Can't find full_screen_button_icon in ui file.");
        full_screen_button_icon.set_property_icon_name(Some("view-fullscreen-symbolic"));

        stack_header.add_named(&media_viewer_headerbar_box, "media-viewer");
        stack_header.set_visible_child_name("media-viewer");

        self.update_media_viewport();
    }

    pub fn save_media(&self) {
        if let Some(ref mv) = self.media_viewer {
            self.save_file_as(mv.image.local_path.lock().unwrap().clone().unwrap_or_default(),
                              (*mv.media_list.read().unwrap())[*mv.current_media_index.read().unwrap()].body.clone());
        }
    }

    pub fn set_zoom_btn_sensitivity(&self) {
        if let Some(ref mv) = self.media_viewer {
            let zoom_out_button = self.ui.builder
                .get_object::<gtk::Button>("zoom_out_button")
                .expect("Cant find zoom_out_button in ui file.");

            let zoom_in_button = self.ui.builder
                .get_object::<gtk::Button>("zoom_in_button")
                .expect("Cant find zoom_in_button in ui file.");

            // FIXME: this is used for waiting for `min_lvl` and `max_lvl` to have their values updated
            // it could be done in a proper way with signals
            gtk::timeout_add(10, clone!(mv => move || match *mv.image.zoom_level.lock().unwrap() {
                None => Continue(true),
                Some(zlvl) => {
                    let min_lvl = mv.zoom_levels.first();
                    let max_lvl = mv.zoom_levels.last();

                    if let Some(min_lvl) = min_lvl {
                        if zlvl <= *min_lvl + FLOATING_POINT_ERROR {
                            zoom_out_button.set_sensitive(false);
                        } else {
                            zoom_out_button.set_sensitive(true);
                        }
                    }

                    if let Some(max_lvl) = max_lvl {
                        if zlvl >= *max_lvl - FLOATING_POINT_ERROR {
                            zoom_in_button.set_sensitive(false);
                        } else {
                            zoom_in_button.set_sensitive(true);
                        }
                    }

                    Continue(false)
                },
            }));
        }
    }

    pub fn update_media_viewport(&mut self) {
        let mut image = None;
        if let Some(ref mv) = self.media_viewer {
            image = Some(self.redraw_image_in_viewport(mv));
        }

        if let Some(ref mut mv) = self.media_viewer {
            if let Some(image) = image {
                mv.image = image;
            }
        }
    }

    pub fn redraw_image_in_viewport(&self, mv: &MediaViewer) -> image::Image {
        let media_viewport = self.ui.builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");

        if let Some(child) = media_viewport.get_child() {
            media_viewport.remove(&child);
        }

        let url = (*mv.media_list.read().unwrap())[*mv.current_media_index.read().unwrap()].url.clone().unwrap_or_default();

        let image = image::Image::new(&self.backend, &url)
                        .fit_to_width(true)
                        .center(true).build();

        media_viewport.add(&image.widget);
        image.widget.show();


        let ui = self.ui.clone();
        let zoom_level = image.zoom_level.clone();
        image.widget.connect_draw(move |_, _| {
            if let Some(zlvl) = *zoom_level.lock().unwrap() {
                update_zoom_entry(&ui, zlvl);
            }

            Inhibit(false)
        });

        self.set_nav_btn_visibility();
        self.set_zoom_btn_sensitivity();

        image
    }
}

fn update_zoom_entry(ui: &uibuilder::UI, zoom_level: f64) {
    let zoom_entry = ui.builder
        .get_object::<gtk::EntryBuffer>("zoom_level")
        .expect("Cant find zoom_level in ui file.");
    zoom_entry.set_text(&format!("{:.0}%", zoom_level * 100.0));
}

fn set_header_title(ui: &uibuilder::UI, title: &str) {
    let media_viewer_headerbar = ui.builder
        .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
        .expect("Cant find media_viewer_headerbar in ui file.");
    media_viewer_headerbar.set_title(title);
}

fn loading_state(ui: &uibuilder::UI, loading_indicator: Arc<RwLock<bool>>, val: bool) {
    let notification: gtk::Revealer = ui.builder
        .get_object("media_viewer_notify_revealer")
        .expect("Can't find media_viewer_notify_revealer in ui file.");
    notification.set_reveal_child(val);

    let previous_media_button = ui.builder
        .get_object::<gtk::Button>("previous_media_button")
        .expect("Cant find previous_media_button in ui file.");
    previous_media_button.set_sensitive(!val);

    let next_media_button = ui.builder
        .get_object::<gtk::Button>("next_media_button")
        .expect("Cant find next_media_button in ui file.");
    next_media_button.set_sensitive(!val);

    *loading_indicator.write().unwrap() = val;
}

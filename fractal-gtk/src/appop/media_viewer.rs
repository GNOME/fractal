extern crate gtk;

use self::gtk::prelude::*;

use uibuilder;
use appop::AppOp;
use appop::AppState;

use widgets::image;

use types::Room;

const FLOATING_POINT_ERROR: f64 = 0.01;

#[derive(Clone)]
pub struct MediaViewer {
    media_names: Vec<String>,
    media_urls: Vec<String>,
    current_media_index: usize,

    image: image::Image,
    zoom_levels: Vec<f64>,
}

impl MediaViewer {
    pub fn new(room: &Room, current_media_url: &str, image: image::Image) -> MediaViewer {
        let img_msgs = room.messages.iter().filter(|msg| msg.mtype == "m.image");
        let media_names: Vec<String> = img_msgs.clone().map(|msg| msg.body.clone()).collect();
        let media_urls: Vec<String> = img_msgs.map(|msg| msg.url.clone().unwrap_or_default()).collect();

        let current_media_index = media_urls.iter().position(|url| url == current_media_url).unwrap_or_default();

        MediaViewer {
            media_names,
            media_urls,
            current_media_index,
            image,
            zoom_levels: vec![0.025, 0.05, 0.1, 0.25, 0.5, 0.75, 1.0],
        }
    }

    pub fn set_zoom_level(&self, zlvl: f64) {
        *self.image.zoom_level.lock().unwrap() = Some(zlvl);
        self.image.widget.queue_draw();
    }
}

impl AppOp {
    pub fn display_media_viewer(&mut self, name: String, url: String, room_id: String) {
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

        set_header_title(&self.ui, &name);

        let media_viewport = self.ui.builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");

        let image = image::Image::new(&self.backend, &url)
                        .fit_to_width(true)
                        .fixed(true).center(true).build();

        media_viewport.add(&image.widget);
        media_viewport.show_all();

        self.media_viewer = Some(MediaViewer::new(r, &url, image.clone()));

        let ui = self.ui.clone();
        let zoom_level = image.zoom_level.clone();
        image.widget.connect_draw(move |_, _| {
            if let Some(zlvl) = *zoom_level.lock().unwrap() {
                update_zoom_entry(&ui, zlvl);
            }

            Inhibit(false)
        });

        self.set_nav_btn_sensitivity();
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
        if let Some(ref mut mv) = self.media_viewer {
            if mv.current_media_index == 0 {
                return;
            }

            mv.current_media_index -= 1;
            let name = &mv.media_names[mv.current_media_index];
            set_header_title(&self.ui, name);
        }

        self.update_media_viewport();
    }

    pub fn next_media(&mut self) {
        if let Some(ref mut mv) = self.media_viewer {
            if mv.current_media_index >= mv.media_urls.len() - 1 {
                return;
            }

            mv.current_media_index += 1;
            let name = &mv.media_names[mv.current_media_index];
            set_header_title(&self.ui, name);
        }

        self.update_media_viewport();
    }

    pub fn set_nav_btn_sensitivity(&self) {
        if let Some(ref mv) = self.media_viewer {
            let previous_media_button = self.ui.builder
                .get_object::<gtk::Button>("previous_media_button")
                .expect("Cant find previous_media_button in ui file.");

            let next_media_button = self.ui.builder
                .get_object::<gtk::Button>("next_media_button")
                .expect("Cant find next_media_button in ui file.");

            if mv.current_media_index == 0 {
                previous_media_button.set_sensitive(false);
            } else {
                previous_media_button.set_sensitive(true);
            }

            if mv.current_media_index >= mv.media_urls.len() - 1 {
                next_media_button.set_sensitive(false);
            } else {
                next_media_button.set_sensitive(true);
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

        stack_header.remove(&media_viewer_headerbar_box);
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
        stack_header.add_named(&media_viewer_headerbar_box, "media-viewer");
        stack_header.set_visible_child_name("media-viewer");

        self.update_media_viewport();
    }

    pub fn save_media(&self) {
        if let Some(ref mv) = self.media_viewer {
            self.save_file_as(mv.image.local_path.lock().unwrap().clone().unwrap_or_default(), mv.media_names[mv.current_media_index].clone());
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

        let url = &mv.media_urls[mv.current_media_index];

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

        self.set_nav_btn_sensitivity();
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

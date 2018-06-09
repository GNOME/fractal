extern crate gtk;

use self::gtk::prelude::*;

use uibuilder;
use appop::AppOp;
use appop::AppState;

use widgets::image;

use types::Room;

#[derive(Clone)]
pub struct MediaViewer {
    media_urls: Vec<String>,
    current_url_index: usize,

    image: image::Image,
}

impl MediaViewer {
    pub fn new(room: &Room, current_media_url: &str, image: image::Image) -> MediaViewer {
        let img_msgs = room.messages.iter().filter(|msg| msg.mtype == "m.image");
        let media_urls: Vec<String> = img_msgs.map(|msg| msg.url.clone().unwrap_or_default()).collect();

        let current_url_index = media_urls.iter().position(|url| url == current_media_url).unwrap_or_default();

        MediaViewer {
            media_urls,
            current_url_index,
            image,
        }
    }

    pub fn set_zoom_level(&self) {
        unimplemented!()
    }
}

impl AppOp {
    pub fn display_media_viewer(&mut self, url: String, room_id: String) {
        let rooms = self.rooms.clone();
        let r = rooms.get(&room_id).unwrap();

        self.set_state(AppState::MediaViewer);

        let media_viewport = self.ui.builder
            .get_object::<gtk::Viewport>("media_viewport")
            .expect("Cant find media_viewport in ui file.");

        let image = image::Image::new(&self.backend,
                                      &url,
                                      None,
                                      image::Thumb(false),
                                      image::Circle(false),
                                      image::Fixed(true),
                                      image::Centered(true));

        media_viewport.add(&image.widget);
        media_viewport.show_all();

        let ui = self.ui.clone();
        let zoom_level = image.zoom_level.clone();
        image.widget.connect_draw(move |_, _| {
            if let Some(zlvl) = *zoom_level.lock().unwrap() {
                update_zoom_entry(&ui, zlvl);
            }

            Inhibit(false)
        });

        self.media_viewer = Some(MediaViewer::new(r, &url, image));

        self.set_nav_btn_sensitivity();
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
            if mv.current_url_index == 0 {
                return;
            }

            mv.current_url_index -= 1;
            let url = &mv.media_urls[mv.current_url_index];

            let media_viewport = self.ui.builder
                .get_object::<gtk::Viewport>("media_viewport")
                .expect("Cant find media_viewport in ui file.");

            if let Some(child) = media_viewport.get_child() {
                media_viewport.remove(&child);
            }

            let image = image::Image::new(&self.backend,
                                          &url,
                                          None,
                                          image::Thumb(false),
                                          image::Circle(false),
                                          image::Fixed(false),
                                          image::Centered(true));

            image.widget.show();
            media_viewport.add(&image.widget);

            let ui = self.ui.clone();
            let zoom_level = image.zoom_level.clone();
            image.widget.connect_draw(move |_, _| {
                if let Some(zlvl) = *zoom_level.lock().unwrap() {
                    update_zoom_entry(&ui, zlvl);
                }

                Inhibit(false)
            });

            mv.image = image;
        }

        self.set_nav_btn_sensitivity();
    }

    pub fn next_media(&mut self) {
        if let Some(ref mut mv) = self.media_viewer {
            if mv.current_url_index >= mv.media_urls.len() - 1 {
                return;
            }

            mv.current_url_index += 1;
            let url = &mv.media_urls[mv.current_url_index];

            let media_viewport = self.ui.builder
                .get_object::<gtk::Viewport>("media_viewport")
                .expect("Cant find media_viewport in ui file.");

            if let Some(child) = media_viewport.get_child() {
                media_viewport.remove(&child);
            }

            let image = image::Image::new(&self.backend,
                                          &url,
                                          None,
                                          image::Thumb(false),
                                          image::Circle(false),
                                          image::Fixed(false),
                                          image::Centered(true));

            image.widget.show();
            media_viewport.add(&image.widget);

            let ui = self.ui.clone();
            let zoom_level = image.zoom_level.clone();
            image.widget.connect_draw(move |_, _| {
                if let Some(zlvl) = *zoom_level.lock().unwrap() {
                    update_zoom_entry(&ui, zlvl);
                }

                Inhibit(false)
            });

            mv.image = image;
        }

        self.set_nav_btn_sensitivity();
    }

    pub fn set_nav_btn_sensitivity(&self) {
        if let Some(ref mv) = self.media_viewer {
            let previous_media_button = self.ui.builder
                .get_object::<gtk::Button>("previous_media_button")
                .expect("Cant find previous_media_button in ui file.");

            let next_media_button = self.ui.builder
                .get_object::<gtk::Button>("next_media_button")
                .expect("Cant find next_media_button in ui file.");

            if mv.current_url_index == 0 {
                previous_media_button.set_sensitive(false);
            } else {
                previous_media_button.set_sensitive(true);
            }

            if mv.current_url_index >= mv.media_urls.len() - 1 {
                next_media_button.set_sensitive(false);
            } else {
                next_media_button.set_sensitive(true);
            }
        }
    }
}

fn update_zoom_entry(ui: &uibuilder::UI, zoom_level: f64) {
    let zoom_entry = ui.builder
        .get_object::<gtk::EntryBuffer>("zoom_level")
        .expect("Cant find zoom_level in ui file.");
    zoom_entry.set_text(&zoom_level.to_string());
}

extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;
use appop::AppState;

use widgets::image;

use types::Room;

pub struct MediaViewer {
    media_urls: Vec<String>,
    current_url_index: usize,
}

impl MediaViewer {
    pub fn from_room(room: &Room, current_media_url: &str) -> MediaViewer {
        let img_msgs = room.messages.iter().filter(|msg| msg.mtype == "m.image");
        let media_urls: Vec<String> = img_msgs.map(|msg| msg.url.clone().unwrap_or_default()).collect();

        let current_url_index = media_urls.iter().position(|url| url == current_media_url).unwrap_or_default();

        MediaViewer {
            media_urls,
            current_url_index,
        }
    }
}

impl AppOp {
    pub fn display_media_viewer(&mut self, url: String, room_id: String) {
        let rooms = self.rooms.clone();
        let r = rooms.get(&room_id).unwrap();
        self.media_viewer = Some(MediaViewer::from_room(r, &url));

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

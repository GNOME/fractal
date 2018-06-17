extern crate gdk;
extern crate gtk;
extern crate gettextrs;

use std::env;
use std::fs;

use self::gtk::prelude::*;
use self::gtk::ResponseType;
use self::gettextrs::gettext;

use glib;

use uibuilder;
use app::App;
use appop::AppOp;
use appop::AppState;

use widgets::image;

use types::Room;

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
            let url = &mv.media_urls[mv.current_media_index];

            set_header_title(&self.ui, name);

            let media_viewport = self.ui.builder
                .get_object::<gtk::Viewport>("media_viewport")
                .expect("Cant find media_viewport in ui file.");

            if let Some(child) = media_viewport.get_child() {
                media_viewport.remove(&child);
            }

            let image = image::Image::new(&self.backend, &url)
                            .fit_to_width(true)
                            .center(true).build();

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
            if mv.current_media_index >= mv.media_urls.len() - 1 {
                return;
            }

            mv.current_media_index += 1;
            let name = &mv.media_names[mv.current_media_index];
            let url = &mv.media_urls[mv.current_media_index];

            set_header_title(&self.ui, name);

            let media_viewport = self.ui.builder
                .get_object::<gtk::Viewport>("media_viewport")
                .expect("Cant find media_viewport in ui file.");

            if let Some(child) = media_viewport.get_child() {
                media_viewport.remove(&child);
            }

            let image = image::Image::new(&self.backend, &url)
                            .fit_to_width(true)
                            .center(true).build();

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
    }

    pub fn enter_full_screen(&self) {
        let main_window = self.ui.builder
            .get_object::<gtk::ApplicationWindow>("main_window")
            .expect("Cant find main_window in ui file.");
        main_window.fullscreen();

        main_window.connect_key_release_event(move |_, k| {
            if k.get_keyval() == gdk::enums::key::Escape {
                APPOP!(leave_full_screen);
            }

            Inhibit(false)
        });
    }

    pub fn leave_full_screen(&self) {
        let main_window = self.ui.builder
            .get_object::<gtk::ApplicationWindow>("main_window")
            .expect("Cant find main_window in ui file.");
        main_window.unfullscreen();
    }

    pub fn save_media(&self) {
        if let Some(ref mv) = self.media_viewer {
            let main_window = self.ui.builder
                .get_object::<gtk::ApplicationWindow>("main_window")
                .expect("Cant find main_window in ui file.");

            let file_chooser = gtk::FileChooserDialog::new(
                Some(&gettext("Save media as")),
                Some(&main_window),
                gtk::FileChooserAction::Save,
            );

            file_chooser.set_modal(true);
            file_chooser.add_buttons(&[
                (&gettext("_Cancel"), ResponseType::Cancel.into()),
                (&gettext("_Save"), ResponseType::Accept.into()),
            ]);
            file_chooser.set_current_folder(env::home_dir().unwrap_or_default());
            file_chooser.set_current_name(&mv.media_names[mv.current_media_index]);

            let local_path = mv.image.local_path.lock().unwrap().clone().unwrap_or_default();
            file_chooser.connect_response(move |fcd, res| {
                if ResponseType::from(res) == ResponseType::Accept {
                    if fcd.get_filename().unwrap_or_default().exists() {
                        let confirm_dialog = gtk::MessageDialog::new(
                            Some(fcd),
                            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
                            gtk::MessageType::Question,
                            gtk::ButtonsType::YesNo,
                            &gettext("Do you want to overwrite the file?")
                        );

                        confirm_dialog.connect_response(clone!(fcd, local_path => move |cd, res| {
                            if ResponseType::from(res) == ResponseType::Yes {
                                if let Err(_) = fs::copy(local_path.clone(), fcd.get_filename().unwrap_or_default()) {
                                    let msg = gettext("Could not save the image");
                                    APPOP!(show_error, (msg));
                                }
                                cd.destroy();
                                fcd.destroy();
                            } else {
                                cd.destroy();
                            }
                        }));

                        confirm_dialog.show_all();
                    } else {
                        if let Err(_) = fs::copy(local_path.clone(), fcd.get_filename().unwrap_or_default()) {
                            let msg = gettext("Could not save the image");
                            APPOP!(show_error, (msg));
                        }
                        fcd.destroy();
                    }
                } else {
                    fcd.destroy();
                }
            });

            file_chooser.show_all();
        }
    }
}

fn update_zoom_entry(ui: &uibuilder::UI, zoom_level: f64) {
    let zoom_entry = ui.builder
        .get_object::<gtk::EntryBuffer>("zoom_level")
        .expect("Cant find zoom_level in ui file.");
    zoom_entry.set_text(&format!("{:3.0}%", zoom_level * 100.0));
}

fn set_header_title(ui: &uibuilder::UI, title: &str) {
    let media_viewer_headerbar = ui.builder
        .get_object::<gtk::HeaderBar>("media_viewer_headerbar")
        .expect("Cant find media_viewer_headerbar in ui file.");
    media_viewer_headerbar.set_title(title);
}

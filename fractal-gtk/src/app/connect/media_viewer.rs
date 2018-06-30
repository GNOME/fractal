extern crate gtk;
extern crate gdk;


use self::gtk::prelude::*;
use self::gdk::WindowExt;
use self::gdk::DisplayExt;
use self::gdk::SeatExt;

use glib;
use std::sync::{Arc, Mutex};

use app::App;

impl App {
    pub fn connect_media_viewer_headerbar(&self) {
        let op = self.op.clone();
        let zoom_entry = self.ui.builder
            .get_object::<gtk::Entry>("zoom_entry")
            .expect("Cant find zoom_entry in ui file.");
        zoom_entry.connect_activate(move |_| {
            op.lock().unwrap().change_zoom_level();
        });

        let op = self.op.clone();
        let zoom_out_button = self.ui.builder
            .get_object::<gtk::Button>("zoom_out_button")
            .expect("Cant find zoom_out_button in ui file.");
        zoom_out_button.connect_clicked(move |_| {
            op.lock().unwrap().zoom_out();
        });

        let op = self.op.clone();
        let zoom_in_button = self.ui.builder
            .get_object::<gtk::Button>("zoom_in_button")
            .expect("Cant find zoom_in_button in ui file.");
        zoom_in_button.connect_clicked(move |_| {
            op.lock().unwrap().zoom_in();
        });

        let op = self.op.clone();
        let full_screen_button = self.ui.builder
            .get_object::<gtk::Button>("full_screen_button")
            .expect("Cant find full_screen_button in ui file.");
        full_screen_button.connect_clicked(move |_| {
            op.lock().unwrap().enter_full_screen();
        });

        let op = self.op.clone();
        let back_btn = self.ui.builder
            .get_object::<gtk::Button>("media_viewer_back_button")
            .expect("Cant find media_viewer_back_button in ui file.");
        back_btn.connect_clicked(move |_| {
            op.lock().unwrap().hide_media_viewer();
        });
    }

    pub fn connect_media_viewer_box(&self) {
        let ui = self.ui.clone();
        let hovered: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

        let previous_media_button = ui.builder
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");

        previous_media_button.connect_enter_notify_event(clone!(hovered => move |_, _| {
            *(hovered.lock().unwrap()) = true;

            Inhibit(false)
        }));
        previous_media_button.connect_leave_notify_event(clone!(hovered => move |_, _| {
            *(hovered.lock().unwrap()) = false;

            Inhibit(false)
        }));

        let next_media_button = ui.builder
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");

        next_media_button.connect_enter_notify_event(clone!(hovered => move |_, _| {
            *(hovered.lock().unwrap()) = true;

            Inhibit(false)
        }));
        next_media_button.connect_leave_notify_event(clone!(hovered => move |_, _| {
            *(hovered.lock().unwrap()) = false;

            Inhibit(false)
        }));

        let media_viewer_box = ui.builder
            .get_object::<gtk::Box>("media_viewer_box")
            .expect("Cant find media_viewer_box in ui file.");

        let source_id: Arc<Mutex<Option<glib::source::SourceId>>> = Arc::new(Mutex::new(None));
        media_viewer_box.connect_motion_notify_event(move |_, _| {
            {
                let mut id = source_id.lock().unwrap();
                if let Some(sid) = id.take() {
                    glib::source::source_remove(sid);
                }
            }

            let main_window = ui.builder
                .get_object::<gtk::Window>("main_window")
                .expect("Cant find main_window in ui file.");
            if let Some(win) = main_window.get_window() {
                if let Some(disp) = gdk::Display::get_default() {
                    if let Some(seat) = disp.get_default_seat() {
                        if let Some(ptr) = seat.get_pointer() {
                            let (_, _, y, _) = win.get_device_position(&ptr);
                            if y <= 6 && win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                                headerbar_revealer.set_reveal_child(true);
                            }
                        }
                    }
                }
            }

            let previous_media_revealer = ui.builder
                .get_object::<gtk::Revealer>("previous_media_revealer")
                .expect("Cant find previous_media_revealer in ui file.");
            previous_media_revealer.set_reveal_child(true);

            let next_media_revealer = ui.builder
                .get_object::<gtk::Revealer>("next_media_revealer")
                .expect("Cant find next_media_revealer in ui file.");
            next_media_revealer.set_reveal_child(true);

            let sid = gtk::timeout_add(1000, clone!(ui, hovered, source_id => move || {
                if !*hovered.lock().unwrap() {
                    let previous_media_revealer = ui.builder
                        .get_object::<gtk::Revealer>("previous_media_revealer")
                        .expect("Cant find previous_media_revealer in ui file.");
                    previous_media_revealer.set_reveal_child(false);

                    let next_media_revealer = ui.builder
                        .get_object::<gtk::Revealer>("next_media_revealer")
                        .expect("Cant find next_media_revealer in ui file.");
                    next_media_revealer.set_reveal_child(false);
                }

                *(source_id.lock().unwrap()) = None;
                gtk::Continue(false)
            }));

            *(source_id.lock().unwrap()) = Some(sid);
            Inhibit(false)
        });

        let op = self.op.clone();
        let previous_media_button = self.ui.builder
            .get_object::<gtk::Button>("previous_media_button")
            .expect("Cant find previous_media_button in ui file.");
        previous_media_button.connect_clicked(move |_| {
            op.lock().unwrap().previous_media();
        });

        let op = self.op.clone();
        let next_media_button = self.ui.builder
            .get_object::<gtk::Button>("next_media_button")
            .expect("Cant find next_media_button in ui file.");
        next_media_button.connect_clicked(move |_| {
            op.lock().unwrap().next_media();
        });

        // Connecting escape key to leave fullscreen mode
        let main_window = self.ui.builder
            .get_object::<gtk::ApplicationWindow>("main_window")
            .expect("Cant find main_window in ui file.");
        let op = self.op.clone();
        main_window.connect_key_release_event(move |w, k| {
            // leave full screen only if we're currently in fullscreen
            if let Some(win) = w.get_window() {
                if !win.get_state().contains(gdk::WindowState::FULLSCREEN) {
                    return Inhibit(false);
                }
            }

            if k.get_keyval() == gdk::enums::key::Escape {
                op.lock().unwrap().leave_full_screen();
            }

            Inhibit(false)
        });
    }
}

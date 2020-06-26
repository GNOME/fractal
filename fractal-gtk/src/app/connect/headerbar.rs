use crate::clone;
use gtk::prelude::*;

use crate::app::App;

impl App {
    pub fn connect_headerbars(&self) {
        if let Some(set) = gtk::Settings::get_default() {
            let left_header: gtk::HeaderBar = self
                .ui
                .builder
                .get_object("left-header")
                .expect("Can't find left-header in ui file.");

            let right_header: gtk::HeaderBar = self
                .ui
                .builder
                .get_object("room_header_bar")
                .expect("Can't find room_header_bar in ui file.");

            if let Some(decor) = set.get_property_gtk_decoration_layout() {
                let decor = decor.to_string();
                let decor_split: Vec<String> =
                    decor.splitn(2, ':').map(|s| s.to_string()).collect();
                // Check if the close button is to the right; If not,
                // change the headerbar controls
                if decor_split.len() > 1 && !decor_split[1].contains("close") {
                    right_header.set_show_close_button(false);
                    left_header.set_show_close_button(true);
                }
            };

            set.connect_property_gtk_decoration_layout_notify(clone!(right_header, left_header, set => move |_| {
                if let Some(decor) = set.get_property_gtk_decoration_layout() {
                    let decor = decor.to_string();
                    let decor_split: Vec<String> = decor.splitn(2,':').map(|s| s.to_string()).collect();
                    // Change the headerbar controls depending on position
                    // of close
                    if decor_split.len() > 1 && decor_split[1].contains("close") {
                        left_header.set_show_close_button(false);
                        right_header.set_show_close_button(true);
                    } else {
                        right_header.set_show_close_button(false);
                        left_header.set_show_close_button(true);
                    }
                };
            }));
        };
    }
}

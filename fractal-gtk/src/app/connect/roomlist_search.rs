use gtk;
use gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_roomlist_search(&self) {
        let op = &self.op;

        let search_btn = self
            .ui
            .builder
            .get_object::<gtk::ToggleButton>("room_search_button")
            .expect("Can't find room_search_button in ui file.");
        let search_bar = self
            .ui
            .builder
            .get_object::<gtk::SearchBar>("room_list_searchbar")
            .expect("Can't find room_list_searchbar in ui file.");
        let search_entry = self
            .ui
            .builder
            .get_object::<gtk::SearchEntry>("room_list_search")
            .expect("Can't find room_list_search in ui file.");

        search_btn
            .bind_property("active", &search_bar, "search-mode-enabled")
            .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
            .build();

        search_entry.connect_search_changed(clone!(op => move |entry| {
            if let Some(text) = entry.get_text() {
                op.lock().unwrap().sidebar_store.filter(&text);
            }
        }));

        // The searchbar has a lef and right box, but we don't want them because we like the search entry
        // to be aligt with the button in the headerbar
        let boxes = search_bar
            .get_child()
            .and_then(|w| w.downcast::<gtk::Revealer>().ok())
            .and_then(|w| w.get_child())
            .and_then(|w| w.downcast::<gtk::Box>().ok());
        if let Some(boxes) = boxes {
            let children = boxes.get_children();
            children[0].hide();
            children[1].set_hexpand(true);
            children[1].set_halign(gtk::Align::Fill);
            children[2].hide();
        }
    }
}

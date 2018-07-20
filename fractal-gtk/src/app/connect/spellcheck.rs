extern crate gtk;
extern crate sourceview;

use gspell;
// use gspell::TextViewExt;
use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_spellcheck(&self) {
        let msg_entry: sourceview::View = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");

        let msg_entry: gtk::TextView = msg_entry.clone().upcast();

        /* Add gspell to the send TextView and enable the basic configuration */
        // if let Some(gspell_text_view) = gspell:TextView::get_from_gtk_text_view(&msg_entry) {
        //     gspell::TextView::basic_set_up(&gspell_entry);
        // }
    }
}

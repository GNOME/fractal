extern crate gtk;

use gspell;
use gspell::EntryExt;
use app::App;

impl App {
    pub fn connect_spellcheck(&self) {
        let msg_entry: gtk::Entry = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");

        /* Add gspell to the send Entry and enable the basic configuration */
        if let Some(gspell_entry) = gspell::Entry::get_from_gtk_entry(&msg_entry) {
            gspell::Entry::basic_setup(&gspell_entry);
        }
    }
}

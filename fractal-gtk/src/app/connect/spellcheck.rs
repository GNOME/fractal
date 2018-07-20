extern crate gtk;
extern crate sourceview;

use gspell;
use gspell::TextBufferExt;
use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_spellcheck(&self) {
        let msg_entry: sourceview::View = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");

        let msg_entry: gtk::TextView = msg_entry.clone().upcast();

        if let Some(buffer) = msg_entry.get_buffer() {
            if let Some(gspell_buffer) = gspell::TextBuffer::get_from_gtk_text_buffer(&buffer) {
                let checker = match gspell::Language::get_default() {
                    None => gspell::Checker::new(None),
                    Some(lang) => gspell::Checker::new(&lang),
                };

                gspell_buffer.set_spell_checker(&checker);
            }
        }
    }
}
